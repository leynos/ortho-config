//! The scan reading this process's own descriptor table.
//!
//! A self-scan is the one case where the diagnostic's own reads appear in the
//! evidence it is reading. It is split from `lock_report_tests` because the cap
//! counts every line a module declares: the report suite plus this case no
//! longer fit in one file.
//!
//! # Why this does not assert "no fault at all"
//!
//! An end-to-end self-scan under the test harness runs in a process whose other
//! threads are opening and closing descriptors of their own, and any of those
//! closing between enumeration and the identity read is a *genuine* incomplete
//! scan — the ordinary event issue #606 records, not a defect. An assertion
//! that a self-scan never reports a fault therefore fails intermittently for a
//! reason that has nothing to do with the code under test, which is precisely
//! the flakiness this PR exists to remove.
//!
//! What *is* deterministic is the rule that makes the self-scan safe: the
//! listing hands back the open iterator it was read through, so the descriptor
//! that iterator owns outlives the entries read beside it. The tests below
//! assert that property without ever counting a table a peer also writes to.

use anyhow::Result;
use std::path::Path;

/// Returns the descriptors of this process open on `directory`.
///
/// Matched by device and inode rather than by link text: a descriptor's target
/// is the path it was opened through, so a directory reached under two names
/// would compare unequal to itself.
#[cfg(target_os = "linux")]
fn descriptors_on(directory: &Path) -> Result<std::collections::BTreeSet<u32>> {
    use anyhow::Context;
    use std::os::unix::fs::MetadataExt;

    let target = std::fs::metadata(directory).context("read the directory's identity")?;
    let wanted = (target.dev(), target.ino());
    let found = std::fs::read_dir("/proc/self/fd")
        .context("list this process's descriptors")?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let descriptor = entry.file_name().into_string().ok()?.parse::<u32>().ok()?;
            let meta = std::fs::metadata(entry.path()).ok()?;
            ((meta.dev(), meta.ino()) == wanted).then_some(descriptor)
        })
        .collect();
    Ok(found)
}

/// Verifies the listing hands back a *live* registry, and that dropping it
/// closes the descriptor it owns.
///
/// The rule is that the iterator is returned rather than dropped, because the
/// descriptor it holds is one a self-scan of the same table would otherwise
/// find closed. Proving it against `/proc/self/fd` itself would mean counting
/// the whole process's descriptor table, which every other test thread adds to
/// and removes from — the count would then fail for reasons that have nothing
/// to do with the code under test, which is how the count-based version of
/// this test failed intermittently.
///
/// A directory unique to this test has no such peers. The registry's descriptor
/// is the only one this process holds on it, so exactly one is visible while
/// the registry is held and none is once it is dropped.
#[cfg(target_os = "linux")]
#[test]
fn the_listing_hands_back_the_registry_it_was_read_through() -> Result<()> {
    use super::fragment_tests::TempRoot;
    use super::lock_procfs::read_listing;
    use anyhow::{Context, ensure};

    let root = TempRoot::new()?;
    std::fs::write(root.path().join("entry"), b"").context("write one entry to be listed")?;

    let listing = read_listing(root.path())
        .map_err(|reason| anyhow::anyhow!("read the test's own directory: {reason}"))?;
    let registry = listing
        .registry
        .expect("the listing must hand back the iterator it was read through");

    let is_held = descriptors_on(root.path())?;
    drop(registry);
    let is_dropped = descriptors_on(root.path())?;

    ensure!(
        is_held.len() == 1,
        "the registry must own exactly one descriptor on the directory while \
         held, so that a scan of the same table still finds it open; saw \
         {is_held:?} — no descriptor means the listing had already let the \
         iterator fall, and more than one means this test is counting a handle \
         some other part of the process opened",
    );
    ensure!(
        is_dropped.is_empty(),
        "dropping the registry must release the descriptor it owned; still open \
         after the drop: {is_dropped:?}",
    );
    Ok(())
}

/// Verifies a self-scan still reports the descriptor *this* process holds open.
///
/// The churn-tolerant half of the property. Whatever unrelated entries the
/// harness closes underneath the scan, the one descriptor this test opened is
/// held for the whole scan, so a complete-and-correct scan must name it. A
/// scan that discarded its matches on an unrelated fault, or that stopped at
/// one, loses it — and that is the defect issue #606 is about, asserted here
/// without depending on whether any transient fault happened to occur.
///
/// The outcome is deliberately not required to be `Ok`: a fault from a peer
/// thread is legitimate. What is required is that the match survives it.
#[cfg(target_os = "linux")]
#[test]
fn a_self_scan_keeps_the_descriptor_this_process_holds() -> Result<()> {
    use super::lock_evidence::ProcessId;
    use super::lock_holder::inspect_descriptors;
    use super::lock_procfs::RealProcFs;
    use anyhow::ensure;
    use std::os::unix::fs::MetadataExt;

    let held_file = std::fs::File::open("/proc/self/stat")
        .expect("this process's own stat file must be openable");
    let own_pid = ProcessId::new(std::process::id());
    let metadata = held_file
        .metadata()
        .expect("an open file must report its identity");
    let identity = super::lock_evidence::LockFileIdentity::of(
        super::lock_evidence::DeviceNumber::from_raw(metadata.dev()),
        super::lock_evidence::Inode::new(metadata.ino()),
    );

    // `held_file` must outlive the scan: closing it before the scan would make
    // its entry unreadable and turn this into the flaky assertion the module
    // docstring explains is not being made.
    let outcome = inspect_descriptors(own_pid, identity, &RealProcFs);
    let matched = match outcome {
        Ok(matched) | Err((matched, _)) => matched,
    };

    ensure!(
        !matched.is_empty(),
        "the descriptor this test is holding open on /proc/self/stat must be \
         matched by a self-scan; an empty result means the scan lost a live \
         descriptor it had read",
    );
    drop(held_file);
    Ok(())
}
