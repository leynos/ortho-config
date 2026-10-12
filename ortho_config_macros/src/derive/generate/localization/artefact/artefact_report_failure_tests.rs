//! Regression tests for the observation channel's own failure record.
//!
//! The channel's contract is that it never fails a build. That contract is
//! only safe because a lost observation is not lost *silently*: the failure
//! goes to a process-scoped record naming the operation and the error. These
//! tests hold that half of the bargain, which the main report suite cannot —
//! it observes successful writes, where nothing is recorded because nothing
//! went wrong.
//!
//! The distinction these tests protect is the one a maintainer needs most: an
//! opted-in build with no report may have had nothing to report, or may have
//! been unable to report it. Only the record tells the two apart.

use std::fs;

use anyhow::{Context, Result, ensure};

use super::artefact_report::{Outcome, Stage, errors_path, report_path, write, write_failure};
use super::fragment_tests::TempRoot;
use super::report_tests::report;

/// Verifies an unwritable channel records its failure instead of going quiet.
///
/// The best-effort contract says a channel failure must not fail the build. It
/// does not say the failure may be invisible: a maintainer who opted in and
/// finds no report cannot tell "nothing to report" from "the report could not
/// be written", and the two call for entirely different responses.
#[test]
fn a_channel_write_failure_is_recorded_rather_than_dropped() -> Result<()> {
    let root = TempRoot::new()?;
    // A *non-empty directory* in the report's position is what makes the
    // atomic write fail: the rename would have to replace that directory with
    // a file, which the kernel refuses. An empty one would not do either —
    // `rename` replaces a directory only with another directory. A regular
    // file would not do it at all, since the rename replaces it happily, and
    // the write would succeed while the test asserted a failure. The report
    // path is process-scoped, so this provokes the failure without disturbing
    // a concurrent test.
    let blocker = report_path(root.path());
    fs::create_dir(&blocker).context("place a directory where the report must go")?;
    fs::write(blocker.join("occupant"), b"keeps the directory non-empty")
        .context("keep the blocking directory non-empty")?;
    write(root.path(), &report(Stage::Merge, Outcome::Failed));

    let recorded = fs::read_to_string(errors_path(root.path()))
        .context("a dropped observation must be recorded")?;
    ensure!(
        recorded.contains("cannot write the build report"),
        "the record must name the operation that failed; got {recorded}",
    );
    Ok(())
}

/// Verifies the failure record names its operation and error.
///
/// The record is the only evidence a maintainer has that the channel dropped
/// something, so it has to say what was lost and why rather than merely that
/// something was.
#[test]
fn the_failure_record_names_the_operation_and_the_error() -> Result<()> {
    let root = TempRoot::new()?;
    write_failure(
        root.path(),
        "cannot write the lock-wait marker",
        &std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    );

    let recorded =
        fs::read_to_string(errors_path(root.path())).context("read the failure record")?;
    ensure!(
        recorded.contains("cannot write the lock-wait marker"),
        "the record must name the failed operation; got {recorded}",
    );
    ensure!(
        recorded.contains("denied"),
        "the record must carry the underlying error; got {recorded}",
    );
    Ok(())
}

/// Verifies repeated channel failures accumulate rather than overwrite.
///
/// A build runs many expansions, and each can lose its own observation. A
/// record that kept only the last would understate a systematic failure as a
/// single incident.
#[test]
fn repeated_channel_failures_accumulate() -> Result<()> {
    let root = TempRoot::new()?;
    write_failure(
        root.path(),
        "first operation",
        &std::io::Error::other("one"),
    );
    write_failure(
        root.path(),
        "second operation",
        &std::io::Error::other("two"),
    );

    let recorded =
        fs::read_to_string(errors_path(root.path())).context("read the failure record")?;
    ensure!(
        recorded.contains("first operation") && recorded.contains("second operation"),
        "every failure must survive in the record; got {recorded}",
    );
    Ok(())
}

/// Verifies retiring a wait marker that was never written is not a failure.
///
/// Absence is the normal case: most publications acquire the lock without
/// waiting at all. Reporting it would fill the error record with noise on
/// every healthy build.
#[test]
fn retiring_an_absent_wait_marker_is_not_recorded() -> Result<()> {
    let root = TempRoot::new()?;
    super::artefact_report::clear_wait(root.path());
    ensure!(
        !errors_path(root.path()).exists(),
        "a missing wait marker is the normal case and must not be reported",
    );
    Ok(())
}
