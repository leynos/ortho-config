//! Regression tests for the observation channel against real publications.
//!
//! The other suites drive the channel's own functions — `write`, `write_failure`,
//! `note_wait` — which pins each file's format and its gating, but not what
//! happens when a *publication* runs with a channel that cannot write. That is
//! the property the best-effort contract is actually about: the channel exists
//! to explain builds and must never change what a build does.
//!
//! These tests therefore call the production `publish::publish` boundary with
//! the report destination blocked, and assert on the publication's own result:
//! a successful build stays successful and still emits its inventory, and a
//! failing build reports the failure it had rather than one the channel
//! introduced. The blocked path is a non-empty directory where the report must
//! go — portable, deterministic, and effective, because the atomic write's
//! final rename cannot replace a directory with a file.
//!
//! The contended case is driven by a real peer holding the real lock rather
//! than by writing a marker on the publisher's behalf, so the announcement
//! under test is one the production acquisition path made while genuinely
//! blocked. The peer's release is requested by the parent, so the test states
//! the ordering rather than racing a sleep.

use std::fs;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};

use super::artefact_channel_log::errors_path;
use super::artefact_lock::lock_path;
use super::artefact_report::{Outcome, ReportConfig, Reporting, Stage, report_path, write};
use super::fragment_tests::TempRoot;
use super::lock_support::{
    ACQUIRE_TIMEOUT, PUBLISH_FINISHED_FILE, PUBLISH_STARTED_FILE, SHORT_WAIT_REFRESH,
    SHORT_WAIT_THRESHOLD, publish_fixture, request_release, spawn_helper, wait_for, wait_for_file,
    wait_for_holder,
};
use super::report_tests::report;

/// Blocks the report's destination so the channel's atomic write must fail.
///
/// A non-empty directory is what does it: `rename` will not replace a
/// directory with a file, so the temporary is written and the rename refused.
/// An empty directory would not do — a rename replaces a directory only with
/// another directory — and a regular file would let the write succeed outright.
fn block_report_destination(root: &std::path::Path) -> Result<()> {
    let blocker = report_path(root);
    fs::create_dir(&blocker).context("place a directory where the report must go")?;
    fs::write(blocker.join("occupant"), b"keeps the directory non-empty")
        .context("keep the blocking directory non-empty")?;
    Ok(())
}

/// Verifies a failed observation leaves a successful publication successful.
///
/// This is the best-effort contract's whole point. A build whose `OUT_DIR` has
/// become unwritable in the report's path — a leftover directory named like the
/// report, a full disk — must still publish its inventory. A channel that
/// turned that into a build failure would be worse than no channel at all, and
/// the failure would surface as a compile error in code the maintainer was only
/// trying to measure.
#[test]
fn a_failed_observation_leaves_a_successful_publication_successful() -> Result<()> {
    let root = TempRoot::new()?;
    block_report_destination(root.path())?;

    publish_fixture(root.path(), ReportConfig::observed(Reporting::On))
        .context("an unwritable report must not fail the publication")?;

    let inventory = fs::read_to_string(root.path().join("cli-identifiers.json"))
        .context("read the inventory the publication wrote")?;
    ensure!(
        inventory.contains("fixture::Config"),
        "the publication's entries must reach the inventory even when its \
         observation could not be written; got {inventory}",
    );
    let recorded = fs::read_to_string(errors_path(root.path()))
        .context("the channel must record the observation it dropped")?;
    ensure!(
        recorded.contains("cannot write the build report"),
        "the dropped observation must be recorded rather than silently lost; got {recorded}",
    );
    Ok(())
}

/// Verifies a failed observation leaves a failed publication failing as before.
///
/// Both directions of the contract matter. If the channel's own failure could
/// replace the publication's, a maintainer would be told the report could not
/// be written — a condition they can neither act on nor reproduce — instead of
/// the real fault. The publication's diagnostic has to survive the channel's.
#[test]
fn a_failed_observation_does_not_mask_a_failed_publication() -> Result<()> {
    let root = TempRoot::new()?;
    let fragment_dir = root.path().join(super::FRAGMENT_DIR);
    fs::write(&fragment_dir, b"not a directory").context("place a file in the directory's path")?;
    block_report_destination(root.path())?;

    let error = publish_fixture(root.path(), ReportConfig::observed(Reporting::On)).expect_err(
        "the publication must still fail for its own reason when its \
             observation also fails",
    );
    ensure!(
        error
            .to_string()
            .contains("cannot create identifier artefact directory"),
        "the publication's own diagnostic must survive the channel's failure; got {error}",
    );
    Ok(())
}

/// Verifies a publication that never took the lock still tries to report.
///
/// The acquisition-failure path writes its report *without* ever holding the
/// artefact lock, so a channel whose report write depended on that lock would
/// drop precisely the observation a maintainer most needs: the build that could
/// not start. Blocking the report destination as well as the lock makes the
/// attempt observable — the write has to be tried, and to fail, on a path where
/// no lock was ever held. A report write that quietly required the lock would
/// leave nothing here to read.
#[test]
fn an_acquisition_failure_still_reports_its_observation() -> Result<()> {
    let root = TempRoot::new()?;
    let blocker = lock_path(root.path());
    fs::create_dir(&blocker).context("place a directory where the lock file must go")?;
    fs::write(blocker.join("occupant"), b"keeps the directory non-empty")
        .context("keep the blocking directory non-empty")?;
    block_report_destination(root.path())?;

    publish_fixture(root.path(), ReportConfig::observed(Reporting::On))
        .expect_err("the publication must fail when its lock cannot be opened");

    let recorded = fs::read_to_string(errors_path(root.path()))
        .context("the acquisition failure's observation must be attempted and recorded")?;
    ensure!(
        recorded.contains("cannot write the build report"),
        "the report write must be attempted on the failed-acquisition path; got {recorded}",
    );
    Ok(())
}

/// Verifies a genuinely blocked publication announces its wait and then retires it.
///
/// Driven by a real peer holding the real lock, so the marker is written by the
/// production acquisition loop while it is genuinely blocked — not by the test
/// on the publisher's behalf. The parent then permits the release, so the
/// ordering is stated rather than raced.
///
/// The refresh is checked as well as the first announcement: the marker is
/// rewritten with a growing wait while the stall continues, which is what lets
/// a maintainer tell a slow peer from a dead one. A single write at the
/// threshold would leave a long stall showing the same small number forever.
#[test]
fn a_blocked_publication_announces_its_wait_and_retires_it() -> Result<()> {
    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold-until-asked")?;
    wait_for_holder(root.path())?;

    let publisher = spawn_helper(root.path(), "publish-observed")?;
    ensure!(
        wait_for_file(root.path(), PUBLISH_STARTED_FILE, ACQUIRE_TIMEOUT),
        "the publisher never reached the publication call within {ACQUIRE_TIMEOUT:?}",
    );

    // The publisher's marker is process-scoped, so the parent waits for it by
    // globbing the stem rather than guessing the child's id.
    let marker = wait_for_marker(root.path(), ACQUIRE_TIMEOUT)
        .context("a blocked observed publisher must announce its wait")?;
    let first = marker
        .get("waited_ms")
        .and_then(serde_json::Value::as_u64)
        .context("the announcement must carry the elapsed wait")?;
    ensure!(
        first >= super::artefact_report::millis(SHORT_WAIT_THRESHOLD),
        "the announcement must carry a wait at or past the threshold; got {first}ms",
    );

    // Wait for a refresh: the same file rewritten with a strictly larger wait.
    let refreshed = wait_for_growth(root.path(), first, ACQUIRE_TIMEOUT)?;
    ensure!(
        refreshed > first,
        "a continuing stall must keep the marker's wait growing; {first}ms never advanced",
    );

    request_release(root.path())?;
    wait_for(publisher, "observed publisher")?;
    wait_for(holder, "holding helper")?;

    ensure!(
        root.path().join(PUBLISH_FINISHED_FILE).exists(),
        "the publisher must complete once the holder releases",
    );
    ensure!(
        marker_path(root.path()).is_none(),
        "the granted lock must retire the marker the wait wrote",
    );
    Ok(())
}

/// Returns the artefact root's sole wait-marker path, if one is there.
///
/// The marker is named for the process that wrote it, and only one helper
/// publishes here, so exactly one marker is expected. The parent globs the stem
/// rather than reconstructing the child's id, which it cannot know before the
/// child writes anything.
fn marker_path(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let raw_name = entry.file_name();
        let Some(name) = raw_name.to_str() else {
            continue;
        };
        let is_json = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
        if name.starts_with(super::artefact_wait_marker::WAIT_STEM) && is_json {
            return Some(path);
        }
    }
    None
}

/// Parses the blocker's current announcement, if one is readable.
///
/// A partially written marker parses as `None` rather than as an error: the
/// marker is replaced atomically, so a reader can catch the rename between
/// reading and writing, and the caller's poll retries.
fn read_announcement(root: &std::path::Path) -> Option<serde_json::Value> {
    let raw = fs::read_to_string(marker_path(root)?).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Waits for a blocked publisher to announce its wait and returns the reading.
fn wait_for_marker(root: &std::path::Path, timeout: Duration) -> Result<serde_json::Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(parsed) = read_announcement(root) {
            return Ok(parsed);
        }
        std::thread::sleep(SHORT_WAIT_REFRESH);
    }
    Err(anyhow::anyhow!(
        "no wait announcement appeared within {timeout:?}"
    ))
}

/// Waits for the marker to be rewritten with a wait greater than `previous`.
fn wait_for_growth(root: &std::path::Path, previous: u64, timeout: Duration) -> Result<u64> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let waited = read_announcement(root)
            .and_then(|parsed| parsed.get("waited_ms").and_then(serde_json::Value::as_u64));
        if waited.is_some_and(|current| current > previous) {
            return waited.context("a growing wait must have a value");
        }
        std::thread::sleep(SHORT_WAIT_REFRESH);
    }
    Err(anyhow::anyhow!(
        "the wait announcement did not grow past {previous}ms within {timeout:?}"
    ))
}

/// Verifies a channel failure on the completion path is recorded once.
///
/// `Observation::finish` is reached by the success path and by every layer an
/// error unwinds through, and its one-shot guard is what keeps one publication
/// from leaving several records of the same lost observation. The guard is
/// already asserted for successful writes; here the write *fails*, which is
/// when a missing guard would multiply the record.
#[test]
fn a_failed_observation_is_recorded_once_per_publication() -> Result<()> {
    let root = TempRoot::new()?;
    block_report_destination(root.path())?;
    write(root.path(), &report(Stage::Prune, Outcome::Completed));

    let recorded =
        fs::read_to_string(errors_path(root.path())).context("read the failure record")?;
    let count = recorded
        .lines()
        .filter(|line| line.contains("cannot write the build report"))
        .count();
    ensure!(
        count == 1,
        "one lost observation must leave exactly one record, not {count}; got {recorded}",
    );
    Ok(())
}
