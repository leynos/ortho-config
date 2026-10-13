//! Cross-process regression tests for the artefact lock primitive.
//!
//! `OUT_DIR` is package-scoped, so a package with several targets can have
//! several compiler processes publishing into one artefact directory. These
//! tests exercise real file locks between real processes rather than
//! in-process threads, because an in-process mutex would satisfy a
//! thread-based test while leaving the compiler-process race intact.
//!
//! These tests cover the `lock` primitive itself. That they also pass with a
//! lock-less `publish` is exactly the gap `publication_tests` closes: proving
//! `lock` excludes peers says nothing about whether publication calls it.
//!
//! The exclusivity test is deliberately not built on wall-clock thresholds. A
//! helper publishes a sentinel while it holds the lock, and the contender reads
//! that sentinel only after acquiring the lock itself. A slow or loaded machine
//! changes how long the contender blocks, never what the contender observes, so
//! the assertion stays meaningful under coverage-instrumented CI.

use std::fs;

use anyhow::{Context, Result, ensure};

use super::artefact_lock::{create_lock, is_free, lock_path};
use super::fragment_tests::TempRoot;
use super::lock_support::{
    SENTINEL_RELEASED, VARIANT_CRATES, sentinel, spawn_helper, spawn_variant, wait_for,
    wait_for_holder,
};

/// Verifies the lock excludes a second process for as long as it is held.
///
/// The holder advertises ownership through the sentinel while still holding the
/// lock. A contender that acquires the lock must therefore observe the released
/// value; observing `SENTINEL_ACQUIRED` proves the two processes overlapped.
#[test]
fn artefact_lock_is_exclusive_across_processes() -> Result<()> {
    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold")?;
    wait_for_holder(root.path())?;

    let contender = spawn_helper(root.path(), "contend")?;
    wait_for(contender, "contending helper")?;
    wait_for(holder, "holding helper")?;
    ensure!(
        sentinel(root.path()).as_deref() == Some(SENTINEL_RELEASED),
        "holder must record its release before exiting",
    );
    Ok(())
}

/// Verifies the lock is released once a publication path returns.
///
/// The probe runs inside the publisher's own process, immediately after the
/// call returns. A parent-side probe taken after the child exits would pass
/// even if publication leaked its lock, because process exit closes the handle.
///
/// This observes the primitive rather than the production sequence: the helper
/// calls `lock` itself, so a lock-less `publish` leaves this test green. That
/// case is covered by `publication_tests`.
#[test]
fn artefact_lock_is_released_after_publication_returns() -> Result<()> {
    let root = TempRoot::new()?;
    let writer = spawn_helper(root.path(), "publish-then-probe")?;
    wait_for(writer, "publishing helper")?;
    Ok(())
}

/// Verifies each publication merges the fragments already on disk.
///
/// This is the read-modify-write the lock exists for. Every child writes only
/// its own fragment, so an inventory that retained a second publisher's entries
/// can only have been produced by re-reading the fragment directory after that
/// peer's fragment landed.
///
/// The children run in sequence rather than concurrently, and deliberately so.
/// The exclusion that makes concurrent publication safe is proved
/// deterministically by `artefact_lock_is_exclusive_across_processes`; racing
/// two children here would add a timing-dependent check whose failure also
/// depends on machine load, so a green run would prove less. Sequenced spawning
/// asserts only what this test is responsible for: that the merge reads the
/// live directory instead of caching a snapshot from before the first write.
#[test]
fn each_publication_merges_the_fragments_already_published() -> Result<()> {
    let root = TempRoot::new()?;
    for variant in VARIANT_CRATES {
        let writer = spawn_variant(root.path(), "publish-variant", Some(variant))?;
        wait_for(writer, &format!("{variant} publishing helper"))?;
    }

    let combined = fs::read_to_string(root.path().join("cli-identifiers.json"))
        .context("read the merged inventory published by both children")?;
    for variant in VARIANT_CRATES {
        ensure!(
            combined.contains(&format!("{variant}::Config")),
            "merged inventory must retain every publisher's entries, but the \
             {variant} entries are missing; a publication that read the \
             fragment directory before its peer wrote would drop them. \
             Inventory: {combined}",
        );
    }
    Ok(())
}

/// Verifies the lock probe does not create the file it reports on.
///
/// The probe answers a question — "is the publication lock held?" — and a
/// query that brings its subject into existence is not answering it. Creating
/// the file also mutates what the next acquirer contends for, and the caller
/// cannot tell the probe's own creation from a peer's.
#[test]
fn probing_a_free_lock_does_not_create_the_lock_file() -> Result<()> {
    let root = TempRoot::new()?;
    let outcome = is_free(root.path());
    ensure!(
        outcome
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
        "a probe over a directory with no lock file must report NotFound rather \
         than create one; got {outcome:?}",
    );
    ensure!(
        !lock_path(root.path()).exists(),
        "the probe must leave no lock file behind",
    );
    Ok(())
}

/// Verifies the probe leaves the lock as free as it found it.
///
/// The probe takes the lock to answer its question, so it must release it
/// before returning. A probe that kept what it acquired would deadlock the
/// next acquirer while reporting the directory free.
#[test]
fn probing_releases_the_lock_it_takes() -> Result<()> {
    let root = TempRoot::new()?;
    create_lock(root.path()).context("create the lock file for the probe")?;

    ensure!(
        is_free(root.path()).context("probe a free lock")?,
        "an unheld lock must be reported free",
    );
    ensure!(
        is_free(root.path()).context("probe the lock a second time")?,
        "a probe must release the lock it took, so a second probe still finds \
         it free",
    );
    Ok(())
}

/// Verifies the probe reports a held lock as held.
///
/// This is the answer the release tests depend on, so it is pinned directly: a
/// probe that always said `false` would make every "the lock was released"
/// assertion fail loudly, but one that always said `true` would make them pass
/// vacuously.
#[test]
fn probing_reports_a_held_lock_as_held() -> Result<()> {
    let root = TempRoot::new()?;
    create_lock(root.path()).context("create the lock file for the holder")?;
    let held = super::artefact_lock::lock(root.path())?;

    ensure!(
        !is_free(root.path()).context("probe the lock this process holds")?,
        "a lock held by this process must be reported as held",
    );
    drop(held);
    ensure!(
        is_free(root.path()).context("probe after releasing the lock")?,
        "releasing the guard must make the lock free again",
    );
    Ok(())
}
