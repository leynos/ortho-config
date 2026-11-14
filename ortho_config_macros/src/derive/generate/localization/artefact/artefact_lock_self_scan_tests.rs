//! The scan reading this process's own descriptor table.
//!
//! A self-scan is the one case where the diagnostic's own reads appear in the
//! evidence it is reading. It is split from `lock_report_tests` because the cap
//! counts every line a module declares: the report suite plus this case no
//! longer fit in one file.

use anyhow::Result;

use super::lock_evidence::ProcessId;

/// Verifies a self-scan does not report a fault it caused itself.
///
/// Listing this process's own descriptor table creates a descriptor holding
/// that listing, and the listing therefore names it. If the scan dropped the
/// listing before it read the identities, that entry would be gone by the time
/// it was read, and the scan would report itself incomplete with no
/// application descriptor having changed. That is precisely the intermittent
/// post-release failure issue #606 records — manufactured by the diagnostic
/// meant to explain it.
///
/// Asserted against a real `/proc/self/fd` read rather than a fake: the double
/// has no descriptor to leak, so only the real implementation can show whether
/// the handle is held long enough.
#[cfg(target_os = "linux")]
#[test]
fn a_self_scan_is_not_made_incomplete_by_its_own_listing() -> Result<()> {
    use super::lock_evidence::{DeviceNumber, Inode, LockFileIdentity};
    use super::lock_holder::inspect_descriptors;
    use super::lock_procfs::RealProcFs;
    use anyhow::ensure;

    let own_pid = ProcessId::new(std::process::id());
    // No descriptor in this process refers to an invented identity, so a
    // complete scan matches nothing — and matching nothing is the point. An
    // incomplete scan is the failure: it would mean the listing's own handle
    // was read after it closed.
    let identity = LockFileIdentity::of(DeviceNumber::new(8, 1), Inode::new(42));

    match inspect_descriptors(own_pid, identity, &RealProcFs) {
        Ok(matched) => {
            ensure!(
                matched.is_empty(),
                "no descriptor of this process refers to an invented identity: {matched:?}",
            );
            Ok(())
        }
        Err((_, reason)) => anyhow::bail!(
            "a self-scan must not report an incomplete scan caused by its own \
             listing handle; the unreadable entry is the scan's own directory \
             iterator: {reason}",
        ),
    }
}
