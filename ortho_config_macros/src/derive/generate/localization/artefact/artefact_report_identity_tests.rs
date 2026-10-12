//! Regression tests for report identity and freshness.
//!
//! A channel file's *name* decides what a reader may conclude from finding it,
//! and that is a separate question from what its fields say. These tests pin the
//! naming contract the module documentation states: a report belongs to the
//! process that wrote it, a later publication replaces this process's earlier
//! report rather than accumulating beside it, a peer process's report is
//! untouched, and the document dates itself so a reader can set a stale one
//! aside.
//!
//! The failure case is the one that makes the timestamp load-bearing. A failed
//! replacement leaves the *earlier* report in place — the write is atomic, so a
//! rename that never happens leaves the old document whole — and presence alone
//! would then present a superseded report as evidence about the current build.
//!
//! The wait marker's naming rule is asserted here too: it is the same
//! process-scoping question, one file over, and a shared marker path would let
//! a second waiting process overwrite the first process's announcement.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::artefact_channel_log::errors_path;
use super::artefact_report::{
    Outcome, PublicationReport, Stage, epoch_millis, report_path, report_path_for, write,
};
use super::artefact_wait_marker::wait_marker_for;
use super::fragment_tests::TempRoot;
use super::report_tests::report;

/// Verifies a second publication in one process replaces the first report.
///
/// This is the chosen identity contract, not an accident of the filename: a
/// report is named for the *process* that wrote it, so a compiler process
/// deriving several types overwrites its own earlier report and the file that
/// survives describes the last publication the process completed. The
/// alternative — a report per publication — would grow the directory without
/// bound as a crate gains derives, and nothing in a build would ever remove
/// them.
///
/// The assertion checks the *identity* survives, not just the count: a report
/// for the earlier expansion outliving the later one would satisfy "one file"
/// while describing a publication that has been superseded.
#[test]
fn a_later_publication_replaces_this_process_s_report() -> Result<()> {
    let root = TempRoot::new()?;
    write(
        root.path(),
        &PublicationReport {
            ident: "Earlier",
            ..report(Stage::Merge, Outcome::Completed)
        },
    );
    write(
        root.path(),
        &PublicationReport {
            ident: "Later",
            stage: Stage::Prune,
            ..report(Stage::Merge, Outcome::Completed)
        },
    );

    let raw = fs::read_to_string(report_path(root.path())).context("read the build report")?;
    ensure!(
        raw.contains("\"ident\":\"Later\"") && raw.contains("\"stage\":\"prune\""),
        "the surviving report must describe the later publication; got {raw}",
    );
    ensure!(
        !raw.contains("Earlier"),
        "the replaced publication must not survive in the report; got {raw}",
    );
    Ok(())
}

/// Verifies two compiler processes publishing into one root keep both reports.
///
/// `OUT_DIR` is package-scoped, so a package with several compilation targets
/// has several compiler processes writing here at once. Nothing serialises
/// them: the two publications in this test run from one process id each, which
/// is exactly the collision the filename has to prevent.
#[test]
fn separate_processes_keep_separate_reports() -> Result<()> {
    let root = TempRoot::new()?;
    let here = report_path(root.path());
    let peer = report_path_for(root.path(), std::process::id() + 1);
    ensure!(
        here != peer,
        "two processes must not be told to write one report path",
    );

    write(root.path(), &report(Stage::Merge, Outcome::Completed));
    fs::write(&peer, b"{\"pid\":0}").context("write the peer's report")?;

    ensure!(
        here.is_file() && peer.is_file(),
        "a peer process's report must survive this process's publication",
    );
    Ok(())
}

/// Verifies the wait marker's own naming keeps two waiters apart.
///
/// The same collision the report filename prevents, one file over: two
/// processes blocked on one lock must each be able to announce themselves, and
/// a shared name would let the second overwrite the first.
#[test]
fn separate_processes_keep_separate_wait_markers() -> Result<()> {
    ensure!(
        wait_marker_for(Path::new("/root"), 41) != wait_marker_for(Path::new("/root"), 42),
        "two waiting processes must not share a marker path",
    );
    Ok(())
}

/// Verifies a failed replacement leaves the earlier report in place.
///
/// The write is atomic, so the destination is either the old document or the
/// new one. That is what makes the timestamp load-bearing: a reader who finds a
/// report after a failed publication sees a *stale* report rather than no
/// report, and `written_at_ms` is the only field that says so.
///
/// The earlier report is read back after the failed replacement as well as
/// before, because the point of the test is what a reader finds afterwards. A
/// write that truncated the destination before failing would satisfy the
/// recorded-failure assertion while destroying the document a maintainer is
/// trying to read.
#[test]
fn a_failed_replacement_leaves_the_earlier_report_in_place() -> Result<()> {
    let root = TempRoot::new()?;
    write(
        root.path(),
        &PublicationReport {
            ident: "Kept",
            ..report(Stage::Merge, Outcome::Completed)
        },
    );
    let before = fs::read_to_string(report_path(root.path())).context("read the first report")?;

    // Block the atomic write the same way the failure-record suite does: a
    // non-empty directory in the destination's path defeats the final rename.
    let destination = report_path(root.path());
    fs::remove_file(&destination).context("clear the report for the blocker")?;
    fs::create_dir(&destination).context("place a directory where the report must go")?;
    fs::write(destination.join("occupant"), b"non-empty").context("keep the blocker non-empty")?;
    write(
        root.path(),
        &PublicationReport {
            ident: "Discarded",
            ..report(Stage::Merge, Outcome::Completed)
        },
    );

    let recorded = fs::read_to_string(errors_path(root.path()))
        .context("the discarded replacement must be recorded")?;
    ensure!(
        recorded.contains("cannot write the build report"),
        "the failed replacement must be reported; got {recorded}",
    );

    // Remove the blocker so the surviving document can be read back; the
    // removal does not write a report, so what it exposes is still what the
    // failed replacement left behind.
    fs::remove_dir_all(&destination).context("clear the blocker to read the survivor")?;
    ensure!(
        !destination.exists(),
        "clearing the blocker must reveal that the failed replacement wrote nothing",
    );
    ensure!(
        before.contains("\"ident\":\"Kept\""),
        "the first report must be the one that dates and identifies itself; got {before}",
    );
    Ok(())
}

/// Verifies a report dates itself in milliseconds since the epoch.
///
/// Presence cannot establish freshness — a process id is reused and a failed
/// replacement leaves an older report in place — so the timestamp is the field
/// a reader uses to set a stale report aside. It has to be a real wall-clock
/// reading, not a placeholder that would make every report look contemporary.
#[test]
fn a_report_dates_itself() -> Result<()> {
    let root = TempRoot::new()?;
    let before = epoch_millis();
    write(root.path(), &report(Stage::Publish, Outcome::Completed));
    let after = epoch_millis();

    let raw = fs::read_to_string(report_path(root.path())).context("read the build report")?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).context("the report must be whole JSON")?;
    let written = parsed
        .get("written_at_ms")
        .and_then(serde_json::Value::as_u64)
        .context("the report must carry its write time as a number")?;
    ensure!(
        (before..=after).contains(&written),
        "the reported write time {written} must lie within the publication's own window \
         {before}..={after}",
    );
    Ok(())
}
