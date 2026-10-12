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
//!
//! The record is read back through `serde_json` rather than by substring
//! matching. Its whole claim is that it is machine-readable — one JSON value
//! per line, under a `.jsonl` extension — so a test that matched text would
//! pass against a file no harness could parse.

use std::fs;

use anyhow::{Context, Result, ensure};

use super::artefact_channel_log::{ERRORS_STEM, errors_path, write_failure};
use super::artefact_report::{ErrorCategory, Outcome, Stage, report_path, write};
use super::artefact_wait_marker::clear_wait;
use super::fragment_tests::TempRoot;
use super::report_tests::report;

/// Reads every record a failure log holds, failing if any line is not JSON.
///
/// A reader consumes the file while a build may still be appending to it, so
/// the parse is per line and a trailing blank line is tolerated; anything else
/// that does not parse is a defect in the format, not in the test.
///
/// Shared with the identity and channel suites: each asserts on the same log, so
/// they read it through one parse rather than three that could disagree about
/// what a malformed line means.
pub(super) fn records(root: &std::path::Path) -> Result<Vec<serde_json::Value>> {
    let raw = fs::read_to_string(errors_path(root)).context("read the failure record")?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .with_context(|| format!("every line must be one JSON value; got {line}"))
        })
        .collect()
}

/// Verifies an unwritable channel records its failure instead of going quiet.
///
/// The best-effort contract says a channel failure must not fail the build. It
/// does not say the failure may be invisible: a maintainer who opted in and
/// finds no report cannot tell "nothing to report" from "the report could not
/// be written", and the two call for entirely different responses.
///
/// A *non-empty directory* in the report's position is what makes the atomic
/// write fail: the rename would have to replace that directory with a file,
/// which the kernel refuses. An empty one would not do either — `rename`
/// replaces a directory only with another directory. A regular file would not
/// do it at all, since the rename replaces it happily, and the write would
/// succeed while the test asserted a failure.
#[test]
fn a_channel_write_failure_is_recorded_rather_than_dropped() -> Result<()> {
    let root = TempRoot::new()?;
    let blocker = report_path(root.path());
    fs::create_dir(&blocker).context("place a directory where the report must go")?;
    fs::write(blocker.join("occupant"), b"keeps the directory non-empty")
        .context("keep the blocking directory non-empty")?;
    write(root.path(), &report(Stage::Merge, Outcome::Failed));

    let recorded = records(root.path())?;
    ensure!(
        recorded.len() == 1,
        "one lost observation must leave exactly one record; got {recorded:?}",
    );
    let record = recorded
        .first()
        .context("the single record read back must be addressable")?;
    let operation = record
        .get("operation")
        .and_then(serde_json::Value::as_str)
        .context("the record must name the operation that failed")?;
    ensure!(
        operation == "cannot write the build report",
        "the record must name the operation that failed; got {operation}",
    );
    ensure!(
        record.get("error").and_then(serde_json::Value::as_str) == Some("io"),
        "a refused rename is an I/O failure and must be classified as one; got {:?}",
        record.get("error"),
    );
    Ok(())
}

/// Verifies the failure record carries the operation, its category, and the
/// underlying error's text.
///
/// The record is the only evidence a maintainer has that the channel dropped
/// something, so it has to say what was lost and why. The category is what a
/// harness groups on; the detail is what a human reads, and is the part the
/// category deliberately does not try to encode.
#[test]
fn the_failure_record_names_the_operation_the_category_and_the_detail() -> Result<()> {
    let root = TempRoot::new()?;
    write_failure(
        root.path(),
        "cannot write the lock-wait marker",
        ErrorCategory::Io,
        &std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    );

    let recorded = records(root.path())?;
    ensure!(recorded.len() == 1, "one failure must leave one record");
    let record = recorded
        .first()
        .context("the single record read back must be addressable")?;
    ensure!(
        record.get("operation").and_then(serde_json::Value::as_str)
            == Some("cannot write the lock-wait marker"),
        "the record must name the failed operation; got {record}",
    );
    ensure!(
        record.get("error").and_then(serde_json::Value::as_str) == Some("io"),
        "the record must carry the bounded category; got {record}",
    );
    ensure!(
        record
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|detail| detail.contains("denied")),
        "the record must carry the underlying error's own text; got {record}",
    );
    ensure!(
        record
            .get("written_at_ms")
            .and_then(serde_json::Value::as_u64)
            .is_some(),
        "the record must date itself in whole milliseconds; got {record}",
    );
    Ok(())
}

/// Verifies a serialization failure is categorised apart from an I/O one.
///
/// The two call for different responses — a full disk versus a report the
/// channel cannot encode — and a log that spelled both the same way would make
/// a harness count them together.
#[test]
fn a_serialization_failure_is_categorised_separately() -> Result<()> {
    let root = TempRoot::new()?;
    write_failure(
        root.path(),
        "cannot serialize the build report",
        ErrorCategory::Serialization,
        &std::fmt::Error,
    );

    let recorded = records(root.path())?;
    let record = recorded
        .first()
        .context("the single record read back must be addressable")?;
    ensure!(
        record.get("error").and_then(serde_json::Value::as_str) == Some("serialization"),
        "a serialization failure must be classified as one; got {recorded:?}",
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
    write_failure(root.path(), "first operation", ErrorCategory::Io, &"one");
    write_failure(root.path(), "second operation", ErrorCategory::Io, &"two");

    let recorded = records(root.path())?;
    ensure!(
        recorded.len() == 2,
        "every lost observation must survive as its own record; got {recorded:?}",
    );
    let operations = recorded
        .iter()
        .map(|record| record.get("operation").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    ensure!(
        operations == [Some("first operation"), Some("second operation")],
        "the records must preserve the order they happened in; got {recorded:?}",
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
    clear_wait(root.path());
    ensure!(
        !errors_path(root.path()).exists(),
        "a missing wait marker is the normal case and must not be reported",
    );
    Ok(())
}

/// Verifies the failure record's extension is the one its format implies.
///
/// The file holds one JSON value per line, which is JSON Lines and not a JSON
/// document. Naming it `.json` was a claim a reader could only disprove by
/// opening it, so the test pins the extension alongside the stem.
#[test]
fn the_failure_record_is_named_for_its_format() -> Result<()> {
    let root = TempRoot::new()?;
    let path = errors_path(root.path());
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("the failure record must have a name")?;
    ensure!(
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl")),
        "a JSON Lines file must not claim to be a JSON document; got {name}",
    );
    ensure!(
        name.starts_with(&format!("{ERRORS_STEM}.")),
        "the failure record must use the documented stem; got {name}",
    );
    Ok(())
}
