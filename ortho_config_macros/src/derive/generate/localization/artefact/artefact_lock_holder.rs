//! Reading a holder's descriptor table out of procfs.
//!
//! The diagnostic report has two halves. `lock_diagnostics` decides what a hold
//! means and what to say about it; this module is the half that goes and asks
//! the kernel. It is split out because the two answer different questions and
//! fail differently: the report must never return an error of its own, while
//! every read here can fail for reasons that are ordinary — a process exits
//! between the lock scan and the descriptor scan, an entry disappears, another
//! user's process is unreadable.
//!
//! Those failures are the reason this half exists as its own type rather than
//! as `Option`s. "This process holds no descriptor on the file" and "this
//! process could not be read" lead to opposite conclusions: only the first is
//! evidence about absence, and even that is read alongside a lock-table record
//! taken at a different moment rather than as proof of inheritance.
//! [`DescriptorScan`] keeps them apart so the report cannot confuse them.
//!
//! Every function here is `#[cfg(target_os = "linux")]`: there is no portable
//! equivalent of `/proc`, and the caller already reports that the holder is
//! unavailable on other platforms.
//!
//! The imports are gated item by item, not as a group, because a group would
//! still be compiled on another target where every user of it has been
//! removed. `describe_process` is the pointed case: it is defined only under
//! `cfg(target_os = "linux")`, so an ungated `use` of it does not merely go
//! unused — it fails to resolve at all.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

#[cfg(target_os = "linux")]
use super::lock_evidence::{Descriptor, DeviceNumber, Inode, LockFileIdentity, ProcessId};
#[cfg(target_os = "linux")]
use super::lock_process::describe_process;

/// Appends one description per process still holding a descriptor on the lock.
#[cfg(target_os = "linux")]
pub(super) fn append_owner_lines(
    lines: &mut Vec<String>,
    owners: &[ProcessId],
    identity: LockFileIdentity,
) {
    let own_pid = ProcessId::new(std::process::id());
    let mut incomplete = 0_usize;
    for &pid in owners {
        let scan = match inspect_holder(pid, identity) {
            Ok(descriptors) => DescriptorScan::Complete(descriptors),
            Err((matched, reason)) => {
                incomplete = incomplete.saturating_add(1);
                DescriptorScan::Incomplete { matched, reason }
            }
        };
        append_owner(lines, pid, scan, own_pid);
    }
    if incomplete > 0 {
        lines.push(format!(
            "holder scan: {incomplete} process(es) could not be read completely, so the descriptor evidence above is incomplete"
        ));
    }
}

/// What a scan of one process's descriptor table found.
///
/// The distinction between "this process holds no descriptor on the file" and
/// "this process could not be read" is the whole point: only the first says the
/// lock survives in an inherited description. Reporting the second as the first
/// would send a maintainer after an innocent process, which is the failure this
/// module exists to prevent.
///
/// An incomplete scan still carries the descriptors it matched before it
/// failed. Those were really observed, so they are evidence of their own; a
/// scan that hit an unreadable entry after matching a descriptor must still
/// name that descriptor, because the alternative is a report that discards
/// live evidence and can no longer explain the hold it was built for.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(super) enum DescriptorScan {
    /// The scan completed and found these descriptors on the lock file.
    Complete(Vec<Descriptor>),
    /// The scan could not be completed, for this reason, after matching these.
    Incomplete {
        /// Descriptors matched before the scan failed. Possibly empty.
        matched: Vec<Descriptor>,
        /// Why the scan could not finish.
        reason: String,
    },
}

/// Describes one process and every descriptor it holds on the lock file.
///
/// An owner with no matching descriptor is reported rather than dropped, but
/// the report does *not* conclude that it inherited the lock. The descriptor
/// scan and the `/proc/locks` record are separate observations taken at
/// different moments, so a complete scan that finds nothing is equally
/// explained by the acquirer having released the lock in between. Naming both
/// possibilities is the honest reading; asserting inheritance would send a
/// maintainer after a process in the ordinary case where the hold had already
/// cleared.
#[cfg(target_os = "linux")]
pub(super) fn append_owner(
    lines: &mut Vec<String>,
    pid: ProcessId,
    scan: DescriptorScan,
    own_pid: ProcessId,
) {
    let described = describe_process(pid, own_pid);
    match scan {
        DescriptorScan::Complete(descriptors) if descriptors.is_empty() => {
            lines.push(format!(
                "holder: {described} has no descriptor on the lock file, so either it released the lock after the lock-table record was read or the lock survives in a description inherited from the recorded acquirer; a descriptor scan is not an atomic snapshot of that record"
            ));
        }
        DescriptorScan::Complete(descriptors) => {
            append_descriptor_lines(lines, &described, descriptors);
        }
        DescriptorScan::Incomplete { matched, reason } => {
            append_descriptor_lines(lines, &described, matched);
            lines.push(format!(
                "holder: {described}'s remaining descriptors could not be read ({reason}), so the scan is incomplete and this report cannot say whether one of them holds the lock file"
            ));
        }
    }
}

/// Appends one line per descriptor a scan matched on the lock file.
#[cfg(target_os = "linux")]
fn append_descriptor_lines(lines: &mut Vec<String>, described: &str, descriptors: Vec<Descriptor>) {
    for descriptor in descriptors {
        lines.push(format!(
            "holder: {described} holds the lock file open as fd {descriptor}"
        ));
    }
}

/// The descriptors one process's table yielded, and whether the scan finished.
///
/// The matched descriptors are reported even when the scan failed. They were
/// observed, so they are evidence; returning only the failure would discard a
/// live holder's descriptor and leave a report that cannot explain the hold.
#[cfg(target_os = "linux")]
type DescriptorScanResult = Result<Vec<Descriptor>, (Vec<Descriptor>, String)>;

/// Lists the descriptors in `pid` that refer to the locked file.
///
/// `/proc/<pid>/fd` is unreadable once the process exits, and unreadable to
/// another user; both are ordinary in a racing system, so they are reported as
/// an incomplete scan rather than a hard failure. An entry whose identity
/// cannot be read is treated the same way: the descriptor list would otherwise
/// silently drop a live holder.
///
/// A failure carries the descriptors matched so far rather than discarding
/// them. During parallel test execution another thread can close an unrelated
/// descriptor between enumeration and the metadata read, which fails an entry
/// that has nothing to do with the lock; a scan that threw away its matches
/// would then report the lock unexplained while the guard was still open. The
/// failure is still a failure — it says the scan is incomplete and makes no
/// claim about absence — but it is no longer a reason to forget what was seen.
///
/// A name that will not parse as a descriptor is skipped rather than treated
/// as a fault. Every entry in this directory is a decimal descriptor number
/// written by the kernel, so a non-numeric name is not a descriptor this scan
/// could have examined and omitted — there is nothing there to hold the lock.
/// An *error* enumerated from the directory is the opposite case: an entry
/// that exists and was not read, which is a partial scan and is reported as
/// one.
#[cfg(target_os = "linux")]
fn inspect_holder(pid: ProcessId, identity: LockFileIdentity) -> DescriptorScanResult {
    inspect_descriptors(&format!("/proc/{pid}/fd"), identity)
}

/// Scans one descriptor directory, which is `/proc/<pid>/fd` outside tests.
///
/// The directory is a parameter so a test can drive this exact loop against a
/// temporary directory. Asserting the loop's behaviour through `append_owner`
/// alone would not exercise the discard this function exists to prevent — a
/// mutant that returned an empty match list on failure would still pass — so
/// the seam is here, at the real `read_dir`, rather than in the report.
#[cfg(target_os = "linux")]
pub(super) fn inspect_descriptors(
    directory: &str,
    identity: LockFileIdentity,
) -> DescriptorScanResult {
    let entries = fs::read_dir(directory)
        .map_err(|error| (Vec::new(), format!("{directory}: unreadable ({error})")))?;
    // Sorted so a failure always leaves the same match list. `read_dir` yields
    // entries in no defined order, so an unsorted scan that hit an unreadable
    // entry would report whichever descriptors happened to precede it — making
    // both the report and any test of it depend on filesystem enumeration order.
    let mut ordered = entries
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| {
            (
                Vec::new(),
                format!("{directory}: an entry could not be read ({error})"),
            )
        })?;
    ordered.sort_by_key(std::fs::DirEntry::file_name);
    let mut descriptors = Vec::new();
    for entry in ordered {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(descriptor) = name.parse().map(Descriptor::new) else {
            continue;
        };
        match refers_to(&entry.path(), identity) {
            Ok(true) => descriptors.push(descriptor),
            Ok(false) => {}
            Err(reason) => {
                return Err((descriptors, format!("{}: {reason}", entry.path().display())));
            }
        }
    }
    Ok(descriptors)
}

/// Reports whether one open descriptor refers to the locked file.
///
/// The entries under `/proc/<pid>/fd` are symlinks, and [`fs::metadata`]
/// follows them, so this reads the metadata of the open file rather than of the
/// link that points at it.
///
/// A read failure is returned rather than folded into `false`: "not this file"
/// and "could not tell" are different answers, and only the first may be used
/// to conclude that a process holds nothing.
#[cfg(target_os = "linux")]
fn refers_to(path: &Path, identity: LockFileIdentity) -> Result<bool, String> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).map_err(|error| format!("unreadable ({error})"))?;
    Ok(identity.holds(
        DeviceNumber::from_raw(metadata.dev()),
        Inode::new(metadata.ino()),
    ))
}
