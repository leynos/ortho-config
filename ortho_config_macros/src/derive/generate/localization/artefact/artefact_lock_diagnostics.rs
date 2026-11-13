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
use super::lock_evidence::{
    DeviceNumber, Inode, LockFileIdentity, LockRecord, ProcessId, parse_lock_record,
};
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
const FLOCK_INHERITANCE_NOTE: &str = concat!(
    "a FLOCK record names the process that acquired the lock, so a ",
    "descriptor inherited by a child keeps it held under this name",
);

/// Asserts that the publication lock below `root` is free, explaining a hold.
///
/// `expectation` is the caller's own sentence about what should have made the
/// lock free, and it stays the first line of the failure so a reader sees the
/// regression before the diagnosis. A held lock is reported with the file it
/// was held on, the current process, every `/proc/locks` record matching that
/// file, whichever process and descriptor still refer to it, and one delayed
/// re-probe that separates "still held" from "released late".
///
/// The report says the lock "is still held" rather than that it *remained* held
/// after a release, because this assertion does not know what released it: its
/// callers include a probe with no guard to drop, so naming a guard release
/// would misdescribe the common case.
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
    ensure_settles_free_observed(root, expectation, &|_| {})
}

/// Asserts the lock settles, reporting every probe to `observe` first.
///
/// [`ensure_settles_free`] with one addition: `observe` is handed each probe
/// as the settle sees it, so a test can act on what the settle saw rather than
/// infer it afterwards. That is what makes a held-to-free test deterministic —
/// releasing *from* an observed `Held` guarantees the settle saw a hold, where
/// a releaser on its own timer only makes that likely. A test that does
/// nothing costs the settle nothing; the hook runs on a path that is about to
/// sleep anyway.
///
/// The hook is not a lock-holding window. [`probe_lock`] drops its own
/// acquisition before it returns, so the lock is already released by the time
/// `observe` runs; what the hook establishes is only that this probe observed
/// the state it reports. A release requested from the hook therefore lands
/// after that observation, but nothing prevents the holder from answering it
/// later than the next [`SETTLE_INTERVAL`] — so a hook must tolerate repeated
/// `Held` probes rather than require the release on the very next one.
///
/// # Errors
///
/// The same failure [`ensure_settles_free`] describes, and the probe's own I/O
/// error when a probe cannot be taken.
pub(super) fn ensure_settles_free_observed(
    root: &Path,
    expectation: &str,
    observe: &dyn Fn(&LockProbe),
) -> Result<()> {
    let deadline = Instant::now() + SETTLE_BUDGET;
    let mut probe = probe_lock(root).with_context(|| expectation.to_owned())?;
    loop {
        observe(&probe);
        let LockProbe::Held { metadata } = probe else {
            return Ok(());
        };
        if Instant::now() >= deadline {
            return Err(report_hold(root, expectation, &metadata));
        }
        thread::sleep(SETTLE_INTERVAL);
        probe = probe_lock(root).with_context(|| expectation.to_owned())?;
    }
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
            "the artefact lock {} is still held",
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
    let inode = Inode::new(metadata.ino());
    let mut lines = vec![
        format!("lock file: {}", lock_path(root).display()),
        format!("device {device}, inode {inode}"),
    ];
    let identity = LockFileIdentity::of(device, inode);
    let owners = append_record_lines(&mut lines, identity);
    super::lock_holder::append_owner_lines(&mut lines, &owners, identity);
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
fn append_record_lines(lines: &mut Vec<String>, identity: LockFileIdentity) -> Vec<ProcessId> {
    let records = match read_records() {
        Ok(records) => records,
        Err(report) => {
            lines.push(report);
            return Vec::new();
        }
    };
    let matching: Vec<&LockRecord> = records
        .iter()
        .filter(|record| record.matches(identity))
        .collect();
    if matching.is_empty() {
        lines.push(format!(
            "{PROC_LOCKS}: no record for this inode; the holder released it between the probe and the scan, or it lives in another pid namespace"
        ));
        return Vec::new();
    }
    let own_pid = ProcessId::new(std::process::id());
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
