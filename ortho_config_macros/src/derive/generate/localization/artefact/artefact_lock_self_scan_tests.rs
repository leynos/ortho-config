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
//! listing hands back the open iterator it was read through, and that iterator
//! owns a descriptor the instant it is dropped. The tests below assert that
//! property directly, on this thread's own descriptors, where no peer's churn
//! can be mistaken for it.

use anyhow::Result;

/// Returns the descriptors of this process open on its own descriptor table.
///
/// `/proc/self` resolves to `/proc/<pid>` when a link is read, so the target is
/// matched by *shape* — `/proc/<digits>/fd` — rather than against the literal
/// `/proc/self/fd`, which never appears.
#[cfg(target_os = "linux")]
fn descriptors_on_the_listing() -> std::collections::BTreeSet<u32> {
    use std::path::Path;

    /// Whether `target` names a process's descriptor-table directory.
    fn is_a_descriptor_table(target: &Path) -> bool {
        let is_fd_directory = target.file_name().is_some_and(|name| name == "fd");
        let parent = target.parent().and_then(Path::file_name);
        is_fd_directory
            && parent.is_some_and(|name| {
                name.to_string_lossy()
                    .chars()
                    .all(|character| character.is_ascii_digit())
            })
    }

    std::fs::read_dir("/proc/self/fd")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| std::fs::read_link(entry.path()).is_ok_and(|t| is_a_descriptor_table(&t)))
        .filter_map(|entry| entry.file_name().into_string().ok()?.parse().ok())
        .collect()
}

/// Verifies the listing hands back a *live* registry, and that dropping it
/// closes the descriptor a self-scan would otherwise find.
///
/// The registry is the only handle whose lifetime decides whether the scan's
/// own entry is readable. Asserting that it is open while held — and gone once
/// dropped — pins both halves of the rule: a `read_listing` that returned
/// `None`, or one that had already let the iterator fall, could not satisfy
/// the first, and a caller that kept the handle alive past its use would still
/// satisfy both but is a leak, not a wrong answer.
///
/// Counted by target rather than by descriptor number: the number is not
/// exposed (and `ReadDir` offers no accessor), but every handle open on
/// `/proc/self/fd` resolves to that same path, so the open ones can be counted
/// from procfs itself. Only this thread opens such handles, so a peer test
/// cannot perturb the count.
#[cfg(target_os = "linux")]
#[test]
fn the_listing_hands_back_the_registry_it_was_read_through() -> Result<()> {
    use super::lock_procfs::read_listing;
    use anyhow::ensure;
    use std::path::Path;

    let listing = read_listing(Path::new("/proc/self/fd"))
        .expect("this process's own descriptor table must be listable");
    let registry = listing
        .registry
        .expect("the listing must hand back the iterator it was read through");

    // The counting helper opens a handle of its own, so the absolute numbers
    // mean nothing; only the difference across the drop does, and the helper
    // is present on both sides.
    let is_held = descriptors_on_the_listing();
    drop(registry);
    let is_dropped = descriptors_on_the_listing();

    ensure!(
        is_held.len() == is_dropped.len().saturating_add(1),
        "the registry must own exactly one descriptor while held and release \
         it on drop; held {} descriptors on /proc/self/fd, after the drop {} \
         — a registry that had already closed would show no difference, and \
         one still open here would be a leak",
        is_held.len(),
        is_dropped.len(),
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
