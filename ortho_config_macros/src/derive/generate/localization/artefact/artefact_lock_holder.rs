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
//! process could not be read" lead to opposite conclusions, and only the first
//! supports saying that the lock survives in a description the holder
//! inherited. [`DescriptorScan`] keeps them apart so the report cannot confuse
//! them.
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
            Err(reason) => {
                incomplete = incomplete.saturating_add(1);
                DescriptorScan::Incomplete(reason)
            }
        };
        append_owner(lines, pid, scan, own_pid);
    }
    if incomplete > 0 {
        lines.push(format!(
            "holder scan: {incomplete} process(es) could not be read, so the descriptor evidence above is incomplete"
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
#[cfg(target_os = "linux")]
enum DescriptorScan {
    /// The scan completed and found these descriptors on the lock file.
    Complete(Vec<Descriptor>),
    /// The scan could not be completed, for this reason.
    Incomplete(String),
}

/// Describes one process and every descriptor it holds on the lock file.
///
/// An owner with no matching descriptor is reported rather than dropped: the
/// lock then survives inside an open file description the process inherited,
/// which is the one case where the recorded pid is not the process to blame.
/// That claim is made only for a [`DescriptorScan::Complete`] scan; an
/// incomplete one says so and names the fault, because the absence it produced
/// is not evidence of inheritance.
#[cfg(target_os = "linux")]
fn append_owner(lines: &mut Vec<String>, pid: ProcessId, scan: DescriptorScan, own_pid: ProcessId) {
    let described = describe_process(pid, own_pid);
    let descriptors = match scan {
        DescriptorScan::Complete(descriptors) => descriptors,
        DescriptorScan::Incomplete(reason) => {
            lines.push(format!(
                "holder: {described}'s descriptors could not be read ({reason}), so the scan is incomplete and this report cannot say whether it holds the lock file"
            ));
            return;
        }
    };
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

/// Lists the descriptors in `pid` that refer to the locked file.
///
/// `/proc/<pid>/fd` is unreadable once the process exits, and unreadable to
/// another user; both are ordinary in a racing system, so they are reported as
/// an incomplete scan rather than a hard failure. An entry whose identity
/// cannot be read is treated the same way: the descriptor list would otherwise
/// silently drop a live holder and read as inheritance.
///
/// A name that will not parse as a descriptor is skipped rather than treated
/// as a fault. Every entry in this directory is a decimal descriptor number
/// written by the kernel, so a non-numeric name is not a descriptor this scan
/// could have examined and omitted — there is nothing there to hold the lock.
/// An *error* enumerated from the directory is the opposite case: an entry
/// that exists and was not read, which is a partial scan and is reported as
/// one.
#[cfg(target_os = "linux")]
fn inspect_holder(pid: ProcessId, identity: LockFileIdentity) -> Result<Vec<Descriptor>, String> {
    let directory = format!("/proc/{pid}/fd");
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{directory}: unreadable ({error})"))?;
    let mut descriptors = Vec::new();
    for candidate in entries {
        let entry = candidate
            .map_err(|error| format!("{directory}: an entry could not be read ({error})"))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(descriptor) = name.parse().map(Descriptor::new) else {
            continue;
        };
        match refers_to(&entry.path(), identity) {
            Ok(true) => descriptors.push(descriptor),
            Ok(false) => {}
            Err(reason) => return Err(format!("{}: {reason}", entry.path().display())),
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    //! Pins the one distinction this module exists to make: a completed scan
    //! that found nothing may claim inheritance, and an unreadable one may not.

    use super::{DescriptorScan, append_owner};
    use crate::derive::generate::localization::artefact::lock_evidence::{Descriptor, ProcessId};
    use rstest::rstest;

    /// Verifies only a completed scan may claim the lock was inherited.
    ///
    /// This is the distinction the module exists for. A report that turns a
    /// failed read into "no descriptor" sends a maintainer after a process that
    /// may be holding the lock, so the incomplete case must say so and must not
    /// use the word that accuses it.
    #[rstest]
    #[case::complete_and_empty(DescriptorScan::Complete(Vec::new()), true, true)]
    #[case::complete_and_holding(
        DescriptorScan::Complete(vec![Descriptor::new(7)]),
        false,
        true
    )]
    #[case::incomplete(
        DescriptorScan::Incomplete("permission denied".to_owned()),
        false,
        false
    )]
    fn only_a_complete_scan_may_report_inheritance(
        #[case] scan: DescriptorScan,
        #[case] expects_inheritance: bool,
        #[case] is_complete: bool,
    ) {
        let own_pid = ProcessId::new(std::process::id());
        let mut lines = Vec::new();
        append_owner(&mut lines, own_pid, scan, own_pid);
        let rendered = lines.join("\n");

        assert_eq!(
            rendered.contains("inherited from the recorded acquirer"),
            expects_inheritance,
            "inheritance must be claimed only by a complete, empty scan: {rendered}",
        );
        assert_eq!(
            rendered.contains("the scan is incomplete"),
            !is_complete,
            "an incomplete scan must say so: {rendered}",
        );
        if !is_complete {
            assert!(
                rendered.contains("permission denied"),
                "an incomplete scan must name the fault it hit: {rendered}",
            );
        }
    }

    /// Verifies a process with no `/proc` entry reports an incomplete scan.
    ///
    /// Unlike the table above, nothing here is injected: this is a real pid
    /// the kernel cannot describe, so the whole path from `append_owner_lines`
    /// — enumeration, failure, counting, and the summary line that says the
    /// evidence is partial — is exercised against procfs itself. A pid of
    /// `u32::MAX` is above `pid_max` on every supported kernel, so the scan
    /// cannot accidentally observe a live process.
    #[test]
    fn a_scan_that_cannot_be_read_reports_the_report_as_incomplete() {
        use super::append_owner_lines;
        use crate::derive::generate::localization::artefact::lock_evidence::{
            DeviceNumber, Inode, LockFileIdentity,
        };

        let missing = ProcessId::new(u32::MAX);
        let identity = LockFileIdentity::of(DeviceNumber::from_raw(0), Inode::new(0));
        let mut lines = Vec::new();
        append_owner_lines(&mut lines, &[missing], identity);
        let rendered = lines.join("\n");

        assert!(
            rendered.contains(&format!("/proc/{missing}/fd")),
            "the report must name the directory it could not read: {rendered}",
        );
        assert!(
            rendered.contains("the scan is incomplete"),
            "an unreadable process must be reported as an incomplete scan: \
             {rendered}",
        );
        assert!(
            rendered.contains("1 process(es) could not be read"),
            "the summary must count the process the scan could not read: \
             {rendered}",
        );
        assert!(
            !rendered.contains("inherited from the recorded acquirer"),
            "an unreadable process must not be accused of inheriting the lock: \
             {rendered}",
        );
    }
}
