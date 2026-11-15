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

use super::artefact_report::{
    Outcome, PublicationReport, Stage, epoch_millis, report_path, report_path_for, write,
};
use super::artefact_wait_marker::wait_marker_for;
use super::fragment_tests::TempRoot;
use super::lock_support::{spawn_observed_publisher, spawn_observed_variant, wait_for};
use super::report_failure_tests::records;
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

/// Verifies one observed publisher's report records its own process and expansion.
///
/// This is the foundation the isolation test builds on: the report has to carry
/// a process id a parent can compare against the child it spawned, and an
/// expansion identity that says which publication the file describes. Without
/// both, "the peer's report survived" would be a claim about file *names*
/// rather than about the documents behind them.
///
/// The publisher is a real process running the production boundary, so the
/// `pid` is the child's rather than this test's, which is what a fabricated
/// report cannot establish.
#[test]
fn an_observed_publisher_records_its_own_process_and_expansion() -> Result<()> {
    let root = TempRoot::new()?;
    let publisher = spawn_observed_publisher(root.path())?;
    let child_pid = publisher.id();
    wait_for(publisher, "observed publisher")?;

    let parsed = read_report(root.path(), child_pid)?;
    ensure_pid(&parsed, child_pid)?;
    ensure!(
        string_field(&parsed, "ident") == Some("Config")
            && string_field(&parsed, "crate_name") == Some("fixture"),
        "the report must name the expansion its publisher derived; got {parsed}",
    );
    Ok(())
}

/// Verifies two real publishers into one root keep both reports.
///
/// `OUT_DIR` is package-scoped, so a package with several compilation targets
/// has several compiler processes writing here at once. Nothing serialises
/// them, so each runs the production publication boundary in its own process
/// and the assertion is on what survived: two process-scoped reports, each
/// naming its own pid and its own expansion.
///
/// A report written by this process on a peer's behalf would prove only that
/// the two *paths* differ, which is not the property — several processes
/// publishing concurrently into one directory is, and only real processes
/// exercise it. The expansions are made distinct by giving each publisher its
/// own variant label, because two publications sharing one identity would
/// legitimately produce one fragment and one described expansion.
#[test]
fn separate_processes_keep_separate_reports() -> Result<()> {
    let root = TempRoot::new()?;
    let first = spawn_observed_variant(root.path(), "alpha")?;
    let first_pid = first.id();
    let second = spawn_observed_variant(root.path(), "beta")?;
    let second_pid = second.id();
    wait_for(first, "first observed publisher")?;
    wait_for(second, "second observed publisher")?;

    let here = report_path_for(root.path(), first_pid);
    let peer = report_path_for(root.path(), second_pid);
    ensure!(
        here != peer,
        "two processes must not be told to write one report path",
    );
    ensure!(
        here.is_file() && peer.is_file(),
        "each publisher's process-scoped report must survive the other's publication",
    );

    let first_report = read_report(root.path(), first_pid)?;
    let second_report = read_report(root.path(), second_pid)?;
    ensure_pid(&first_report, first_pid)?;
    ensure_pid(&second_report, second_pid)?;
    ensure!(
        string_field(&first_report, "crate_name") == Some("alpha")
            && string_field(&second_report, "crate_name") == Some("beta"),
        "each report must name the expansion its own process derived; got \
         {first_report} and {second_report}",
    );
    Ok(())
}

/// Reads the report one process wrote, parsed rather than matched as text.
fn read_report(root: &Path, pid: u32) -> Result<serde_json::Value> {
    let raw = fs::read_to_string(report_path_for(root, pid))
        .with_context(|| format!("read the report process {pid} wrote"))?;
    serde_json::from_str(&raw).with_context(|| format!("the report for {pid} must be whole JSON"))
}

/// Requires a parsed report to name the process that wrote it.
fn ensure_pid(parsed: &serde_json::Value, pid: u32) -> Result<()> {
    ensure!(
        parsed.get("pid").and_then(serde_json::Value::as_u64) == Some(u64::from(pid)),
        "the report must carry the writing process's own id {pid}; got {parsed}",
    );
    Ok(())
}

/// Returns one string field of a parsed report, if it is a string.
fn string_field<'a>(parsed: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    parsed.get(name).and_then(serde_json::Value::as_str)
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
/// The failure is placed on the write's temporary rather than on its
/// destination, and that is the whole point. `atomic_write` writes a sibling
/// and then renames it over the destination, so blocking the temporary fails
/// the replacement while the earlier report sits untouched throughout — the
/// state the test exists to observe. Blocking the destination would first
/// require deleting the report to put a directory in its place, which destroys
/// the evidence and leaves an assertion that the old document is absent
/// standing in for one that it survives.
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
    let destination = report_path(root.path());
    let before = fs::read(&destination).context("read the first report")?;

    // The temporary is a non-empty directory, so writing the replacement into
    // it fails outright and the rename is never reached.
    let temporary = destination.with_extension("tmp");
    fs::create_dir(&temporary).context("place a directory where the temporary must go")?;
    fs::write(temporary.join("occupant"), b"keeps the directory non-empty")
        .context("keep the blocking directory non-empty")?;
    write(
        root.path(),
        &PublicationReport {
            ident: "Discarded",
            ..report(Stage::Merge, Outcome::Completed)
        },
    );

    let recorded = records(root.path()).context("the discarded replacement must be recorded")?;
    ensure!(
        recorded.iter().any(|record| {
            string_field(record, "operation") == Some("cannot write the build report")
                && string_field(record, "error") == Some("io")
        }),
        "the failed replacement must leave a structured record of what was lost; got {recorded:?}",
    );

    let after = fs::read(&destination)
        .context("the earlier report must still be at its own destination")?;
    ensure!(
        after == before,
        "a failed replacement must leave the earlier report byte for byte; got {}",
        String::from_utf8_lossy(&after),
    );
    ensure!(
        String::from_utf8_lossy(&after).contains("\"ident\":\"Kept\""),
        "the document that survives must be the one written before the failure",
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
