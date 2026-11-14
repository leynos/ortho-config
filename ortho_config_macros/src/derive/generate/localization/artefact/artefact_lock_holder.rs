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
use super::lock_evidence::{Descriptor, LockFileIdentity, ProcessId};
#[cfg(target_os = "linux")]
use super::lock_process::describe_process;
#[cfg(target_os = "linux")]
use super::lock_procfs::ProcFs;

/// Appends one description per process still holding a descriptor on the lock.
///
/// Reads through `procfs` rather than the filesystem directly, so a caller can
/// fail one scan and leave the rest answering. See [`super::lock_procfs`] for
/// why those reads are worth a seam.
#[cfg(target_os = "linux")]
pub(super) fn append_owner_lines(
    lines: &mut Vec<String>,
    owners: &[ProcessId],
    identity: LockFileIdentity,
    procfs: &dyn ProcFs,
) {
    let own_pid = ProcessId::new(std::process::id());
    let mut incomplete = 0_usize;
    for &pid in owners {
        let scan = match inspect_holder(pid, identity, procfs) {
            Ok(descriptors) => DescriptorScan::Complete(descriptors),
            Err((matched, reason)) => {
                incomplete = incomplete.saturating_add(1);
                DescriptorScan::Incomplete { matched, reason }
            }
        };
        append_owner(
            lines,
            OwnerScan {
                pid,
                scan,
                own_pid,
                procfs,
            },
        );
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

/// One process's scan, together with what the report needs to describe it.
///
/// The pid, the scan taken of it, and the identity of the process writing the
/// report always travel together: a scan is meaningless without knowing whose
/// table it came from, and every line naming a holder says how that holder
/// relates to the reader. Passing them separately let a caller pair one
/// process's scan with another's name, which is the same transposition the
/// evidence newtypes exist to prevent — at one remove.
#[cfg(target_os = "linux")]
pub(super) struct OwnerScan<'a> {
    /// The process this scan describes.
    pub(super) pid: ProcessId,
    /// What the scan of that process found.
    pub(super) scan: DescriptorScan,
    /// The process writing the report, for the relation it states.
    pub(super) own_pid: ProcessId,
    /// The procfs the description of `pid` is read through.
    pub(super) procfs: &'a dyn ProcFs,
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
pub(super) fn append_owner(lines: &mut Vec<String>, owner: OwnerScan<'_>) {
    let OwnerScan {
        pid,
        scan,
        own_pid,
        procfs,
    } = owner;
    let described = describe_process(pid, own_pid, procfs);
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
fn inspect_holder(
    pid: ProcessId,
    identity: LockFileIdentity,
    procfs: &dyn ProcFs,
) -> DescriptorScanResult {
    inspect_descriptors(pid, identity, procfs)
}

/// Scans one process's descriptor table for descriptors on the locked file.
///
/// The reads go through [`ProcFs`] so a test can drive this exact loop against a
/// fake table. Asserting the loop's behaviour through `append_owner` alone would
/// not exercise the discard this function exists to prevent — a mutant that
/// returned an empty match list on failure would still pass — so the seam is
/// here, at the reads themselves, rather than in the report.
///
/// The scan takes the process rather than a directory path because the two
/// reads it makes — list the table, then identify each descriptor — must agree
/// on *which* process they describe. Passing a path let a caller scan one
/// directory and read identities from another, which no longer type-checks.
///
/// A name that is not a descriptor number is *skipped*, not treated as a fault:
/// every entry the kernel writes is a decimal number, so a name that will not
/// parse is not a descriptor this scan could have examined and omitted. Failing
/// on it would mark every process incompletely scanned and drown the real
/// faults. It is still counted in the report's summary, because a name the scan
/// did not understand is a gap in its coverage even when it is not an error.
#[cfg(target_os = "linux")]
pub(super) fn inspect_descriptors(
    pid: ProcessId,
    identity: LockFileIdentity,
    procfs: &dyn ProcFs,
) -> DescriptorScanResult {
    let entries = procfs
        .descriptor_entries(pid)
        .map_err(|reason| (Vec::new(), reason))?;
    let mut descriptors = Vec::new();
    for &descriptor in entries.descriptors() {
        match procfs.descriptor_identity(pid, descriptor) {
            Ok(seen) if seen == identity => descriptors.push(descriptor),
            Ok(_) => {}
            Err(reason) => {
                // The name the read failed on is part of the evidence: without
                // it the report says only that something could not be read.
                return Err((descriptors, format!("fd {descriptor}: {reason}")));
            }
        }
    }
    Ok(descriptors)
}
