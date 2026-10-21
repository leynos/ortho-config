//! Diagnosis of a publication lock that still looks held after its release.
//!
//! The lock probe answers a boolean: free or held. That is enough to fail a
//! test and useless for explaining it, because the same `held` is produced by a
//! handle this process leaked and by an unrelated process holding the same
//! inode — and the two call for opposite fixes. This module turns a
//! [`LockProbe::Held`] into a report that names the lock file, the process, and
//! the descriptor still owning the lock, so the intermittent post-release
//! failures recorded in issue #606 can be diagnosed from the failure text.
//!
//! Everything here is test support. It reads ambient procfs rather than a
//! capability handle, and it must never be able to turn one failure into a
//! different one: an unreadable `/proc` entry degrades the report, it never
//! replaces the primary error.
//!
//! `/proc/locks` is a racing snapshot of the whole machine, filtered to the
//! reading process's pid namespace. A record can therefore be missing for a
//! lock that is genuinely held, and a record that is present can be stale by
//! the time it is read. [`recheck_line`] re-probes after a short delay for that
//! reason, and the report says what it observed rather than implying a
//! certainty the kernel does not offer.
//!
//! The evidence vocabulary — the device split and the lock-table parser — lives
//! in `artefact_lock_evidence`, because it is pure and testable everywhere,
//! while the procfs scan below it only exists on Linux.

use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};

use super::artefact_lock::{LockProbe, lock_path, probe_lock};
#[cfg(target_os = "linux")]
use super::lock_evidence::{DeviceNumber, LockRecord, parse_lock_record};
use super::lock_support::SHORT_WAIT_THRESHOLD;

/// Where the kernel publishes its lock table on Linux.
#[cfg(target_os = "linux")]
const PROC_LOCKS: &str = "/proc/locks";

/// How long the delayed re-probe waits before looking at the lock again.
///
/// The suite's existing short-wait constant rather than a new one: it is
/// already small enough to sit inside a failing test's budget and long enough
/// for a lock released a moment late to be observed as free.
const RECHECK_DELAY: Duration = SHORT_WAIT_THRESHOLD;

/// How long a release assertion waits for a fork-inherited description to clear.
///
/// A child holds the inherited description only from `fork` until `exec`, and
/// `exec` is reached immediately by the short-lived commands this test tree
/// spawns. The budget is generous relative to that window because it is paid
/// only on a path that would otherwise fail, and it stops well short of the
/// twenty-second acquire timeout so a genuine leak still fails promptly.
pub(super) const SETTLE_BUDGET: Duration = Duration::from_secs(2);

/// How long each settling probe waits before looking at the lock again.
const SETTLE_INTERVAL: Duration = Duration::from_millis(10);

/// Explains why an `FLOCK` record can name this process while a child owns it.
///
/// `flock` locks belong to the open file description, and a record reports the
/// process that *acquired* the lock rather than every process that inherited
/// the descriptor. A child this test spawned therefore keeps the lock alive
/// while `/proc/locks` still names the parent — the first thing to suspect when
/// the reported holder is the process doing the reporting.
#[cfg(target_os = "linux")]
const FLOCK_INHERITANCE_NOTE: &str = "a FLOCK record names the process that acquired the lock, so a \
     descriptor inherited by a child keeps it held under this name";

/// Asserts that the publication lock below `root` is free, explaining a hold.
///
/// `expectation` is the caller's own sentence about what should have made the
/// lock free, and it stays the first line of the failure so a reader sees the
/// regression before the diagnosis. A held lock is reported with the file it
/// was held on, the current process, every `/proc/locks` record matching that
/// file, whichever process and descriptor still refer to it, and one delayed
/// re-probe that separates "still held" from "released late".
///
/// # Errors
///
/// Returns the primary assertion failure when the lock is held, and the probe's
/// own I/O error when the probe cannot be taken. Diagnostic faults are folded
/// into the report text instead of being returned, so a failure to *explain* a
/// hold can never mask the hold itself.
pub(super) fn ensure_released(root: &Path, expectation: &str) -> Result<()> {
    let probe = probe_lock(root).with_context(|| expectation.to_owned())?;
    match probe {
        LockProbe::Free => Ok(()),
        LockProbe::Held { metadata } => Err(report_hold(root, expectation, &metadata)),
    }
}

/// Asserts that the lock becomes free within a bounded settling window.
///
/// This is the assertion a *gate* should use, and [`ensure_released`] is the
/// one that explains why it failed. The difference is what `flock` means when a
/// peer forks: the lock belongs to the open file description, which a child
/// inherits across `fork` and loses only at `exec`. A probe taken in that window
/// therefore reports the lock held although its owner has released it, and no
/// amount of care at the release site can prevent that — the release is correct
/// and the observation is early.
///
/// That window is transient by construction, so waiting it out separates the two
/// cases a single probe cannot: a lock that is still held because a descriptor
/// leaked, and a lock that merely outlived its owner by one fork. The former
/// survives every probe in [`SETTLE_BUDGET`] and fails with the full report; the
/// latter settles. Both outcomes keep the caller's `expectation` as the first
/// line.
///
/// [`SETTLE_BUDGET`]: self::SETTLE_BUDGET
///
/// # Errors
///
/// Returns the same failure [`ensure_released`] describes once the lock is still
/// held at the end of the window, and the probe's own I/O error when a probe
/// cannot be taken.
pub(super) fn ensure_settles_free(root: &Path, expectation: &str) -> Result<()> {
    let deadline = Instant::now() + SETTLE_BUDGET;
    let mut probe = probe_lock(root).with_context(|| expectation.to_owned())?;
    while let LockProbe::Held { metadata } = probe {
        if Instant::now() >= deadline {
            return Err(report_hold(root, expectation, &metadata));
        }
        thread::sleep(SETTLE_INTERVAL);
        probe = probe_lock(root).with_context(|| expectation.to_owned())?;
    }
    Ok(())
}

/// Builds the failure a still-held lock produces.
///
/// The caller's expectation leads, every line below it is one observation, and
/// nothing here can fail: a report that could return an error of its own would
/// have to choose between two failures at the one moment both matter.
fn report_hold(root: &Path, expectation: &str, metadata: &fs::Metadata) -> anyhow::Error {
    let mut lines = vec![
        expectation.to_owned(),
        format!(
            "the artefact lock {} is still held after its guard was released",
            lock_path(root).display()
        ),
    ];
    lines.extend(holder_lines(root, metadata));
    lines.push(recheck_line(root));
    anyhow!(lines.join("\n"))
}

/// Reports what the platform can say about the lock's holder.
#[cfg(target_os = "linux")]
fn holder_lines(root: &Path, metadata: &fs::Metadata) -> Vec<String> {
    use std::os::unix::fs::MetadataExt;

    let device = DeviceNumber::from_raw(metadata.dev());
    let inode = metadata.ino();
    let mut lines = vec![
        format!("lock file: {}", lock_path(root).display()),
        format!("device {device}, inode {inode}"),
    ];
    let owners = append_record_lines(&mut lines, device, inode);
    append_owner_lines(&mut lines, &owners, device, inode);
    lines
}

/// Reports that this platform has no lock-table equivalent of `/proc/locks`.
///
/// The file and its size are still named: a reader switching platforms should
/// see that the lock was genuinely held, and see that only the *holder* is
/// unavailable rather than the evidence.
#[cfg(not(target_os = "linux"))]
fn holder_lines(root: &Path, metadata: &fs::Metadata) -> Vec<String> {
    vec![format!(
        "holder diagnostics are Linux-only: {} ({} bytes) is held, but this platform cannot report which process holds it",
        lock_path(root).display(),
        metadata.len()
    )]
}

/// Re-probes the lock once after a short delay and reports what changed.
///
/// A hold that clears by itself is a different defect from one that persists:
/// the first points at a descriptor closed later than intended, the second at a
/// live owner. One delayed observation separates them for the cost of a single
/// short sleep, paid only on an already-failing path.
fn recheck_line(root: &Path) -> String {
    thread::sleep(RECHECK_DELAY);
    match probe_lock(root) {
        Ok(LockProbe::Free) => format!("re-probe after {RECHECK_DELAY:?}: now free"),
        Ok(LockProbe::Held { .. }) => format!("re-probe after {RECHECK_DELAY:?}: still held"),
        Err(error) => format!("re-probe after {RECHECK_DELAY:?}: {error}"),
    }
}

/// Appends the lock-table records matching the lock file, returning their pids.
///
/// Blocked records share their blocker's inode, so they are printed — they show
/// a peer waiting — but their pid is never treated as an owner: a waiter does
/// not hold anything.
#[cfg(target_os = "linux")]
fn append_record_lines(lines: &mut Vec<String>, device: DeviceNumber, inode: u64) -> Vec<u32> {
    let records = match read_records() {
        Ok(records) => records,
        Err(report) => {
            lines.push(report);
            return Vec::new();
        }
    };
    let matching: Vec<&LockRecord> = records
        .iter()
        .filter(|record| record.matches(device, inode))
        .collect();
    if matching.is_empty() {
        lines.push(format!(
            "{PROC_LOCKS}: no record for this inode; the holder released it between the probe and the scan, or it lives in another pid namespace"
        ));
        return Vec::new();
    }
    let own_pid = std::process::id();
    let mut owners = Vec::new();
    let mut explained_inheritance = false;
    for record in matching {
        lines.push(format!("{PROC_LOCKS}: {}", record.describe()));
        owners.extend(record.owning_pid());
        let is_own_flock = record.is_flock() && record.owning_pid() == Some(own_pid);
        if is_own_flock && !explained_inheritance {
            lines.push(format!("{PROC_LOCKS}: {FLOCK_INHERITANCE_NOTE}"));
            explained_inheritance = true;
        }
    }
    // A peer waiting on the lock contributes no owner, and several records can
    // name the same acquirer for different ranges, so the list is deduplicated
    // before each pid is scanned.
    owners.sort_unstable();
    owners.dedup();
    owners
}

/// Reads and parses the kernel's lock table.
///
/// A failed read is returned as text rather than an error: the caller is
/// already reporting a failure, and a diagnosis that could not be gathered must
/// appear *inside* that report.
#[cfg(target_os = "linux")]
fn read_records() -> Result<Vec<LockRecord>, String> {
    fs::read_to_string(PROC_LOCKS)
        .map(|table| table.lines().filter_map(parse_lock_record).collect())
        .map_err(|error| format!("{PROC_LOCKS}: unreadable ({error})"))
}

/// Appends one description per process still holding a descriptor on the lock.
#[cfg(target_os = "linux")]
fn append_owner_lines(lines: &mut Vec<String>, owners: &[u32], device: DeviceNumber, inode: u64) {
    let own_pid = std::process::id();
    let mut skipped = 0_usize;
    for &pid in owners {
        match inspect_holder(pid, device, inode) {
            Ok(descriptors) => append_owner(lines, pid, &descriptors, own_pid),
            Err(reason) => {
                skipped = skipped.saturating_add(1);
                lines.push(format!("holder pid {pid}: {reason}"));
            }
        }
    }
    if skipped > 0 {
        lines.push(format!(
            "holder scan: skipped {skipped} process(es) whose descriptors could not be read"
        ));
    }
}

/// Describes one process and every descriptor it holds on the lock file.
///
/// An owner with no matching descriptor is reported rather than dropped: the
/// lock then survives inside an open file description the process inherited,
/// which is the one case where the recorded pid is not the process to blame.
#[cfg(target_os = "linux")]
fn append_owner(lines: &mut Vec<String>, pid: u32, descriptors: &[u32], own_pid: u32) {
    let described = describe_process(pid, own_pid);
    if descriptors.is_empty() {
        lines.push(format!(
            "holder: {described} has no descriptor on the lock file, so the lock survives in a description inherited from the recorded acquirer"
        ));
        return;
    }
    for descriptor in descriptors {
        lines.push(format!(
            "holder: {described} holds the lock file open as fd {descriptor}"
        ));
    }
}

/// Names a process by pid, command, parent, and relation to the reporting one.
#[cfg(target_os = "linux")]
pub(super) fn describe_process(pid: u32, own_pid: u32) -> String {
    match process_identity(pid) {
        Some(identity) => format!(
            "pid {pid} ({}), parent {}, {}",
            identity.command,
            identity.parent,
            relation(pid, Some(identity.parent), own_pid)
        ),
        None => format!(
            "pid {pid} (<stat unreadable>), {}",
            relation(pid, None, own_pid)
        ),
    }
}

/// Labels a pid by how it relates to the process building the report.
#[cfg(target_os = "linux")]
const fn relation(pid: u32, parent: Option<u32>, own_pid: u32) -> &'static str {
    if pid == own_pid {
        return "this process";
    }
    match parent {
        Some(reported) if reported == own_pid => "child of this process",
        Some(_) => "other",
        None => "other (parent unknown)",
    }
}

/// What `/proc/<pid>/stat` says about one process.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(super) struct ProcessIdentity {
    /// The command name the kernel records, truncated to 15 bytes by `comm`.
    pub(super) command: String,
    /// The pid of the process that forked this one.
    pub(super) parent: u32,
}

/// Reads a process's command name and parent pid from `/proc/<pid>/stat`.
///
/// The command name sits in parentheses and may itself contain spaces or
/// parentheses, so the fields after it are located from the *last* closing
/// parenthesis rather than by splitting the whole line: splitting would make a
/// process called `a) b` look like a different process entirely.
#[cfg(target_os = "linux")]
pub(super) fn process_identity(pid: u32) -> Option<ProcessIdentity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let command = stat.get(open.saturating_add(1)..close)?.to_owned();
    let mut trailing = stat.get(close.saturating_add(1)..)?.split_whitespace();
    // Past the command name the first field is the state and the second the
    // parent pid.
    let _state = trailing.next()?;
    let parent = trailing.next()?.parse().ok()?;
    Some(ProcessIdentity { command, parent })
}

/// Lists the descriptors in `pid` that refer to the locked file.
///
/// `/proc/<pid>/fd` is unreadable once the process exits, and unreadable to
/// another user; both are ordinary in a racing system, so they are returned as
/// a reason the caller records and counts instead of a hard failure.
#[cfg(target_os = "linux")]
fn inspect_holder(pid: u32, device: DeviceNumber, inode: u64) -> Result<Vec<u32>, String> {
    let directory = format!("/proc/{pid}/fd");
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{directory}: unreadable ({error})"))?;
    let mut descriptors = Vec::new();
    for candidate in entries {
        let Ok(entry) = candidate else { continue };
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(descriptor) = name.parse::<u32>() else {
            continue;
        };
        if refers_to(&entry.path(), device, inode) {
            descriptors.push(descriptor);
        }
    }
    Ok(descriptors)
}

/// Reports whether one open descriptor refers to the locked file.
///
/// The entries under `/proc/<pid>/fd` are symlinks, and [`fs::metadata`]
/// follows them, so this reads the metadata of the open file rather than of the
/// link that points at it.
#[cfg(target_os = "linux")]
fn refers_to(path: &Path, device: DeviceNumber, inode: u64) -> bool {
    use std::os::unix::fs::MetadataExt;

    fs::metadata(path).is_ok_and(|metadata| {
        metadata.ino() == inode && DeviceNumber::from_raw(metadata.dev()) == device
    })
}
