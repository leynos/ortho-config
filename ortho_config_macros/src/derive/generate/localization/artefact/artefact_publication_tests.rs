//! Regression tests that observe the production publication sequence.
//!
//! `artefact_lock_tests` proves the `lock` primitive excludes peers. It does
//! not prove that publication *uses* it: every one of those tests either takes
//! the lock itself or probes after the publisher has already exited, so a
//! `publish` that never locked — or that released before writing — leaves them
//! all green. These tests close that gap by driving the real `publish::publish`
//! boundary and observing it from another process.
//!
//! Two independent observations carry the proof:
//!
//! * A peer holds the lock while an announcing publisher is launched. The
//!   publisher must not complete during that window. Skipping the lock lets it
//!   run straight through, which the test detects without any timing guess
//!   beyond "a blocked process does not finish".
//! * A publisher probes its own lock state after returning, on both the success
//!   and failure paths. A leaked guard fails both probes, and unlike a
//!   parent-side probe the check cannot be satisfied by process exit closing
//!   the handle.
//!
//! Both are deterministic rather than load-sensitive: a slow machine widens
//! how long a publisher stays blocked, never whether it was blocked.

use std::fs;

use anyhow::{Context, Result, ensure};

use super::artefact_lock::lock_path;
use super::artefact_report::{ReportConfig, Reporting, report_path};
use super::artefact_wait_marker::{WAIT_WARN_THRESHOLD, note_wait, wait_marker};
use super::fragment_tests::TempRoot;
use super::lock_support::{
    ACQUIRE_TIMEOUT, BLOCKED_WINDOW, PUBLISH_FINISHED_FILE, PUBLISH_STARTED_FILE,
    RELEASE_REQUEST_FILE, publish_fixture, request_release, spawn_helper, wait_for, wait_for_file,
    wait_for_holder,
};

/// Verifies publication takes the lock before it writes anything.
///
/// A peer process holds the lock for a window the parent controls. The
/// publisher is then launched and announces its arrival at the publication
/// call. Because a lock-honouring publisher blocks at that point, completion
/// inside the window is impossible unless the production sequence never
/// acquired the lock — which is the regression this asserts against.
///
/// The window is bounded by the parent's release request rather than by a
/// sleep, so the only assumption is that a blocked process does not return.
#[test]
fn publication_waits_for_a_peer_holding_the_lock() -> Result<()> {
    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold-until-asked")?;
    wait_for_holder(root.path())?;

    let publisher = spawn_helper(root.path(), "publish-announced")?;
    // The publisher has announced that it reached the publication call, so it
    // is either blocked on the lock or already past it.
    ensure!(
        wait_for_file(root.path(), PUBLISH_STARTED_FILE, ACQUIRE_TIMEOUT),
        "the publisher never reached the publication call within {ACQUIRE_TIMEOUT:?}",
    );
    let escaped = wait_for_file(root.path(), PUBLISH_FINISHED_FILE, BLOCKED_WINDOW);
    // Release the holder before asserting, so a failure does not leave the
    // publisher blocked until its own timeout expires.
    request_release(root.path())?;
    ensure!(
        !escaped,
        "publication completed while a peer still held the lock, so the \
         production sequence did not acquire it; a peer's fragment write and \
         merge could interleave with this one and drop entries",
    );

    wait_for(publisher, "announcing publisher")?;
    wait_for(holder, "holding helper")?;
    Ok(())
}

/// Verifies a publisher releases the lock before its call returns.
///
/// The probe runs inside the publisher's own process straight after
/// `publish::publish` returns, so a guard that outlives the call — a leaked
/// `mem::forget`, or a guard held past the publication step — fails here. A
/// parent-side probe taken after the child exits would not, because process
/// exit closes the handle.
#[test]
fn publication_releases_the_lock_before_returning() -> Result<()> {
    let root = TempRoot::new()?;
    let writer = spawn_helper(root.path(), "publish-then-probe")?;
    wait_for(writer, "publishing helper")?;
    Ok(())
}

/// Verifies the failure path releases the lock, and did take it first.
///
/// A publication that fails after locking must still release, because the
/// compiler proceeds to other expansions and a stuck lock would block every
/// later process. The failure is injected at the metadata probe, which runs
/// after the fragment write and therefore after the lock is taken, so this
/// exercises the error path with the guard already held.
///
/// The helper asserts the failure came from the injection. Without that, an
/// incidental early error would also produce an error outcome and the test
/// would pass while never reaching the lock at all.
#[test]
fn publication_releases_the_lock_after_a_failed_publication() -> Result<()> {
    let root = TempRoot::new()?;
    let writer = spawn_helper(root.path(), "publish-failing-then-probe")?;
    wait_for(writer, "failing publishing helper")?;
    Ok(())
}

/// Verifies a peer can publish once the blocking holder releases.
///
/// Exclusivity alone would also be satisfied by a publisher that never
/// proceeds, so this pins the other half: after the holder releases, a waiting
/// publisher completes and its entries reach the merged inventory. A build
/// where publication deadlocked, or discarded its fragment on release, would
/// fail here even though the exclusivity test passed.
#[test]
fn publication_proceeds_once_the_holder_releases() -> Result<()> {
    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold-until-asked")?;
    wait_for_holder(root.path())?;

    let publisher = spawn_helper(root.path(), "publish-announced")?;
    ensure!(
        wait_for_file(root.path(), PUBLISH_STARTED_FILE, ACQUIRE_TIMEOUT),
        "the publisher never reached the publication call within {ACQUIRE_TIMEOUT:?}",
    );
    request_release(root.path())?;

    wait_for(publisher, "announcing publisher")?;
    wait_for(holder, "holding helper")?;

    ensure!(
        root.path().join(PUBLISH_FINISHED_FILE).exists(),
        "the publisher must record completion after the holder released",
    );
    let inventory = fs::read_to_string(root.path().join("cli-identifiers.json"))
        .context("read the inventory the blocked publisher eventually wrote")?;
    ensure!(
        inventory.contains("fixture::Config"),
        "the publisher's entries must reach the merged inventory after the \
         holder releases, but the inventory holds: {inventory}",
    );
    Ok(())
}

/// Verifies an unwritable fragment directory is reported as the fragment stage.
///
/// Creating the fragment directory is part of writing the fragment. Reporting
/// it as [`Stage::LockWait`] would tell a maintainer the build was blocked on a
/// peer when the lock had already been granted and the failure was local. The
/// stage is recorded before the directory is created precisely so this path
/// cannot be misattributed.
#[test]
fn a_fragment_directory_failure_reports_the_fragment_stage() -> Result<()> {
    let root = TempRoot::new()?;
    let fragment_dir = root.path().join(super::FRAGMENT_DIR);
    // A regular file where the fragment directory belongs makes `create_dir_all`
    // fail with `AlreadyExists` on every supported platform, so the provoked
    // failure is the same one a maintainer would see from a layout clash.
    fs::write(&fragment_dir, b"not a directory").context("place a file in the directory's path")?;

    publish_fixture(root.path(), ReportConfig::observed(Reporting::On))
        .expect_err("a publication must fail when its fragment directory cannot be created");

    let raw = fs::read_to_string(report_path(root.path()))
        .context("the failed publication must still record its report")?;
    ensure!(
        raw.contains("\"stage\":\"fragment_write\""),
        "a fragment-directory failure must be reported as the fragment stage, \
         not as a lock wait; got {raw}",
    );
    ensure!(
        raw.contains("\"outcome\":\"failed\""),
        "the report must record the failure; got {raw}",
    );
    ensure!(
        raw.contains("\"error\":\"io\""),
        "the report must classify a directory failure as I/O; got {raw}",
    );
    Ok(())
}

/// Verifies a completed publication leaves no wait marker behind.
///
/// The marker means "this process is blocked on the lock right now". One that
/// outlives its wait makes the next maintainer diagnose a stall that has
/// already ended, and the file is process-scoped so nothing else will clear it.
#[test]
fn a_completed_publication_retires_its_wait_marker() -> Result<()> {
    let root = TempRoot::new()?;
    // The marker is written directly because a real threshold-length stall
    // would make this test take the production five seconds; what is under test
    // is the retirement, not the announcement.
    note_wait(root.path(), WAIT_WARN_THRESHOLD);
    ensure!(
        wait_marker(root.path()).exists(),
        "the fixture must place the marker before the publication runs",
    );

    publish_fixture(root.path(), ReportConfig::observed(Reporting::On))?;

    ensure!(
        !wait_marker(root.path()).exists(),
        "a publication that acquired the lock must retire the marker it wrote",
    );
    Ok(())
}

/// Verifies a failed acquisition retires the wait marker too.
///
/// Acquisition failure is the other terminal path, and it never reaches the
/// grant that normally retires the marker. Leaving it would make a build that
/// could not take the lock look like a build still waiting for it — the
/// opposite of what happened, and the failure a maintainer most needs stated
/// plainly.
///
/// **The lock file is what must be unopenable, not the fragment directory.**
/// Blocking the fragment directory fails *after* the lock was granted, so it
/// exercises `Observation::granted`'s retirement and leaves `abandon_wait`
/// unreached: the test would pass with `abandon_wait` deleted, which is
/// precisely the defect it exists to catch. A directory in the lock file's
/// place makes `open_lock_file` fail before any lock is held.
///
/// A *non-empty* directory is what makes the open fail portably: opening a
/// directory for writing is refused, and a regular file would be opened
/// happily, taking a lock the publication would then succeed with while the
/// test asserted a failure.
#[test]
fn a_failed_acquisition_retires_its_wait_marker() -> Result<()> {
    let root = TempRoot::new()?;
    let blocker = lock_path(root.path());
    fs::create_dir(&blocker).context("place a directory where the lock file must go")?;
    fs::write(blocker.join("occupant"), b"keeps the directory non-empty")
        .context("keep the blocking directory non-empty")?;
    note_wait(root.path(), WAIT_WARN_THRESHOLD);
    ensure!(
        wait_marker(root.path()).exists(),
        "the fixture must place the marker before the publication runs",
    );

    let error = publish_fixture(root.path(), ReportConfig::observed(Reporting::On))
        .expect_err("a publication whose lock file cannot be opened must fail");
    ensure!(
        error
            .to_string()
            .contains("cannot open identifier artefact lock"),
        "the failure must be the acquisition's own diagnostic, so this test cannot \
         pass by failing somewhere later; got {error}",
    );

    ensure!(
        !wait_marker(root.path()).exists(),
        "a terminal publication must not leave a wait marker describing a wait \
         that has ended",
    );
    let raw = fs::read_to_string(report_path(root.path()))
        .context("the failed acquisition must still record its report")?;
    ensure!(
        raw.contains("\"stage\":\"lock_wait\""),
        "the failure happened before the lock was granted, so the report must name \
         the lock wait; got {raw}",
    );
    ensure!(
        raw.contains("\"outcome\":\"failed\""),
        "the report must record the failure; got {raw}",
    );
    ensure!(
        raw.contains("\"error\":\"io\""),
        "an unopenable lock file is an I/O failure; got {raw}",
    );
    Ok(())
}

/// Keeps the release request distinguishable from the publication markers.
///
/// The parent writes `RELEASE_REQUEST_FILE` to let a holder go, and all three
/// markers live in the artefact directory. Were the release request to share a
/// name with the completion marker, an unblocked publisher could be reported as
/// a blocked one and the blocking assertion above would go vacuously green.
#[test]
fn release_request_is_distinct_from_publication_markers() {
    assert_ne!(RELEASE_REQUEST_FILE, PUBLISH_STARTED_FILE);
    assert_ne!(RELEASE_REQUEST_FILE, PUBLISH_FINISHED_FILE);
    assert_ne!(PUBLISH_STARTED_FILE, PUBLISH_FINISHED_FILE);
}
