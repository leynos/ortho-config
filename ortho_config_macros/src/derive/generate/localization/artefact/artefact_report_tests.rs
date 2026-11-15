//! Regression tests for the opt-in build-time observation channel.
//!
//! The channel exists because a procedural macro has no subscriber a build
//! maintainer can configure, so these tests check the properties that make it
//! usable: a quiet build writes nothing at all, an opted-in build records the
//! stage, durations, and outcome of the publication it observed, and an
//! opted-in build that cannot write its observations says so instead of
//! looking like a build with nothing to report.
//!
//! No assertion here depends on how long anything took. A duration overrun is
//! reported, not asserted, and the wait marker is driven through its own
//! reporting function rather than by keeping a publisher genuinely blocked for
//! the production threshold.
//!
//! What a report says about *identity and freshness* — that a later publication
//! replaces this process's earlier one, that two processes keep their own
//! reports, and that a report dates itself — lives in
//! [`artefact_report_identity_tests`](super::report_identity_tests), so the
//! document's own format and each contract about its lifetime can be read
//! separately.

use std::fs;
use std::time::Duration;

use anyhow::{Context, Result, ensure};

use super::artefact_channel_log::{ERRORS_STEM, errors_path};
use super::artefact_report::{
    ErrorCategory, Outcome, PublicationReport, REPORT_ENV, REPORT_STEM, ReportConfig, Reporting,
    Stage, WAIT_WARN_THRESHOLD, report_path, write,
};
use super::artefact_wait_marker::{WAIT_STEM, wait_marker};
use super::fragment_tests::TempRoot;
use super::lock_support::publish_fixture;

/// Builds a report for one stage and outcome.
///
/// Shared with the failure-record suite as `super::report_tests::report`, so
/// both describe the same document rather than each holding its own idea of
/// what a report looks like.
pub(super) fn report(stage: Stage, outcome: Outcome) -> PublicationReport<'static> {
    PublicationReport {
        pid: std::process::id(),
        crate_name: "fixture",
        ident: "Config",
        source_file: "/fixture-source.rs",
        stage,
        outcome,
        error: None,
        lock_wait: Duration::from_millis(3),
        elapsed: Duration::from_millis(9),
        written_at_ms: super::artefact_report::epoch_millis(),
    }
}

/// Verifies only the documented literal opts a build into the channel.
///
/// Emission and observation deliberately share one opt-in convention (`"1"`),
/// so a build that writes an inventory can ask why it was slow without
/// learning a second spelling. Every other value must stay quiet: a channel
/// that switched itself on for `true` or `yes` would start writing files into
/// artefact directories of builds that never requested them.
#[test]
fn reporting_opt_in_requires_literal_one() -> Result<()> {
    ensure!(Reporting::requested(None) == Reporting::Off);
    ensure!(Reporting::requested(Some("0")) == Reporting::Off);
    ensure!(Reporting::requested(Some("true")) == Reporting::Off);
    ensure!(Reporting::requested(Some("")) == Reporting::Off);
    ensure!(Reporting::requested(Some("1")) == Reporting::On);
    ensure!(!ReportConfig::quiet().reporting.is_on());
    ensure!(!Reporting::Off.is_on() && Reporting::On.is_on());
    Ok(())
}

/// Verifies the report records the stage, outcome, and both durations.
///
/// The stage is the field that makes a failure actionable: "cannot render"
/// alone does not say whether the lock was ever acquired, so the report has to
/// carry it alongside a bounded outcome rather than only free-text error
/// detail.
#[test]
fn the_report_records_its_stage_durations_and_outcome() -> Result<()> {
    let root = TempRoot::new()?;
    write(root.path(), &report(Stage::Merge, Outcome::Failed));

    let raw = fs::read_to_string(report_path(root.path())).context("read the build report")?;
    ensure!(
        raw.contains("\"stage\":\"merge\""),
        "the report must name the stage the publication reached; got {raw}",
    );
    ensure!(
        raw.contains("\"outcome\":\"failed\""),
        "the report must carry a bounded outcome category; got {raw}",
    );
    ensure!(
        raw.contains("\"lock_wait_ms\":3"),
        "the report must record the lock wait; got {raw}",
    );
    ensure!(
        raw.contains("\"elapsed_ms\":9"),
        "the report must record the total duration; got {raw}",
    );
    ensure!(
        raw.contains("\"crate_name\":\"fixture\""),
        "the report must name the crate whose expansion published; got {raw}",
    );
    Ok(())
}

/// Verifies a report names the process and expansion it describes.
///
/// `OUT_DIR` is package-scoped, so one artefact directory can hold reports
/// from several compiler processes compiling several targets. A report that
/// named only its crate would leave a maintainer unable to tell which
/// publication it belongs to, or whether the file is stale output from an
/// earlier build that never got cleaned.
#[test]
fn the_report_names_the_process_and_expansion_it_describes() -> Result<()> {
    let root = TempRoot::new()?;
    write(root.path(), &report(Stage::Prune, Outcome::Completed));

    let raw = fs::read_to_string(report_path(root.path())).context("read the build report")?;
    let pid = std::process::id();
    ensure!(
        raw.contains(&format!("\"pid\":{pid}")),
        "the report must name the publishing process; got {raw}",
    );
    ensure!(
        raw.contains("\"ident\":\"Config\""),
        "the report must name the deriving type; got {raw}",
    );
    ensure!(
        raw.contains("\"source_file\":\"/fixture-source.rs\""),
        "the report must name the expansion's source file; got {raw}",
    );
    Ok(())
}

/// Verifies a failed report carries a bounded failure category.
///
/// The stage says where a build died and the category says what went wrong
/// there, so a harness can group failures without parsing compiler prose. A
/// successful publication carries neither: an always-present field would
/// invite a reader to treat it as meaningful on every row.
#[test]
fn only_a_failed_report_carries_an_error_category() -> Result<()> {
    let root = TempRoot::new()?;
    write(
        root.path(),
        &PublicationReport {
            error: Some(ErrorCategory::Serialization),
            ..report(Stage::Merge, Outcome::Failed)
        },
    );
    let failed = fs::read_to_string(report_path(root.path())).context("read the failed report")?;
    ensure!(
        failed.contains("\"error\":\"serialization\""),
        "a failed report must carry its bounded category; got {failed}",
    );

    write(root.path(), &report(Stage::Publish, Outcome::Completed));
    let completed =
        fs::read_to_string(report_path(root.path())).context("read the completed report")?;
    ensure!(
        !completed.contains("\"error\""),
        "a completed report must omit the failure category entirely; got {completed}",
    );
    Ok(())
}

/// Verifies every stage has a distinct serialized name.
///
/// A reader that has to interpret the report mechanically cannot afford two
/// stages collapsing to one token, which is exactly what a handwritten
/// `Display` would risk; deriving the spelling from the enum keeps the wire
/// contract and the code in step.
#[test]
fn every_stage_serializes_to_a_distinct_name() -> Result<()> {
    let mut seen = Vec::new();
    for stage in [
        Stage::LockWait,
        Stage::FragmentWrite,
        Stage::Merge,
        Stage::Render,
        Stage::Prune,
        Stage::Publish,
    ] {
        let encoded = serde_json::to_string(&stage).context("serialize a publication stage")?;
        ensure!(
            !seen.contains(&encoded),
            "stage {stage:?} must not reuse the name of an earlier stage",
        );
        seen.push(encoded);
    }
    Ok(())
}

/// Verifies no channel file is mistaken for rendered inventory.
///
/// Pruning deletes rendered files the current render does not produce, so a
/// channel filename that matched the artefact matcher would be deleted on the
/// next publication and the report would silently vanish. Every name the
/// channel writes therefore has to sit outside that matcher — including the
/// failure record, whose `.jsonl` extension is what keeps its second stem from
/// being read as an index.
#[test]
fn channel_files_are_not_rendered_artefacts() -> Result<()> {
    for name in [
        format!("{REPORT_STEM}.1234.json"),
        format!("{WAIT_STEM}.1234.json"),
        format!("{ERRORS_STEM}.1234.jsonl"),
        REPORT_STEM.to_owned(),
        WAIT_STEM.to_owned(),
        ERRORS_STEM.to_owned(),
    ] {
        ensure!(
            !super::is_rendered_artefact(&name),
            "{name} must not be pruned as superseded inventory",
        );
    }
    Ok(())
}

/// Verifies a quiet build gains no channel file anywhere in its artefact root.
///
/// This is the property that lets the channel ship: a normal build must not
/// acquire unexplained files in the directory it publishes, because a consumer
/// globbing that directory would otherwise start seeing artefacts it never
/// asked for. The assertion drives a real publication rather than checking the
/// opt-in predicate alone, so a report written outside the gate fails here.
#[test]
fn a_quiet_build_writes_no_report() -> Result<()> {
    let root = TempRoot::new()?;
    let quiet = ReportConfig::quiet();
    ensure!(
        !quiet.reporting.is_on(),
        "a default configuration must not enable reporting",
    );
    ensure!(
        quiet.threshold == WAIT_WARN_THRESHOLD,
        "the default configuration must be the documented threshold",
    );

    publish_fixture(root.path(), quiet)?;

    ensure!(
        !report_path(root.path()).exists(),
        "a quiet build must not write a build report",
    );
    ensure!(
        !wait_marker(root.path()).exists(),
        "a quiet build must not write a wait marker",
    );
    ensure!(
        !errors_path(root.path()).exists(),
        "a quiet build must not write an observation-error record",
    );
    Ok(())
}

/// Verifies an opted-in publication records the stage it completed.
///
/// The channel's purpose is to say where a build spent its time and where it
/// failed, so a successful run must still leave a report naming the final
/// stage. Checking the outcome alone would let a report that always claims
/// failure pass.
#[test]
fn an_opted_in_publication_reports_completion() -> Result<()> {
    let root = TempRoot::new()?;
    let observed = ReportConfig::observed(Reporting::On);
    publish_fixture(root.path(), observed)?;

    let raw = fs::read_to_string(report_path(root.path())).context("read the build report")?;
    ensure!(
        raw.contains("\"outcome\":\"completed\""),
        "an uninterrupted publication must report completion; got {raw}",
    );
    ensure!(
        raw.contains("\"stage\":\"publish\""),
        "the final stage of a completed publication is the publish step; got {raw}",
    );
    ensure!(
        !wait_marker(root.path()).exists(),
        "a completed publication must retire its wait marker",
    );
    Ok(())
}

/// Verifies a report survives atomically rather than in place.
///
/// The report is read by a harness that may glob the directory while another
/// publication runs, so it must appear whole or not at all. A partial JSON
/// document would make a reader report a parse failure for a healthy build.
#[test]
fn the_report_is_written_atomically() -> Result<()> {
    let root = TempRoot::new()?;
    write(root.path(), &report(Stage::Prune, Outcome::Completed));
    let temporary = report_path(root.path()).with_extension("tmp");
    ensure!(
        !temporary.exists(),
        "the temporary used for the atomic write must not be left behind",
    );
    let raw = fs::read_to_string(report_path(root.path())).context("read the build report")?;
    serde_json::from_str::<serde_json::Value>(&raw).context("the report must be whole JSON")?;
    Ok(())
}

/// Verifies the channel's opt-in name and every file stem are documented ones.
///
/// These names are the channel's whole interface for a build maintainer, so a
/// rename that no test noticed would silently make the documented globs match
/// nothing.
#[test]
fn the_channel_names_are_the_documented_ones() -> Result<()> {
    ensure!(REPORT_ENV == "ORTHO_CONFIG_ARTEFACT_REPORT");
    ensure!(REPORT_STEM == "cli-identifiers.build-report");
    ensure!(WAIT_STEM == "cli-identifiers.lock-wait");
    ensure!(
        ERRORS_STEM == "cli-identifiers.observation-errors",
        "the observation-error stem must be the documented one",
    );
    Ok(())
}
