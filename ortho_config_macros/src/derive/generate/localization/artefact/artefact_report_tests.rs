//! Regression tests for the opt-in build-time observation channel.
//!
//! The channel exists because a procedural macro has no subscriber a build
//! maintainer can configure, so these tests check the two properties that make
//! it usable: a quiet build writes nothing at all, and an opted-in build
//! records the stage, durations, and outcome of the publication it observed.
//!
//! No assertion here depends on how long anything took. A duration overrun is
//! reported, not asserted, and the wait marker is driven through its own
//! reporting function rather than by keeping a publisher genuinely blocked for
//! the production threshold.

use std::fs;
use std::time::Duration;

use anyhow::{Context, Result, ensure};

use super::artefact_report::{
    Outcome, PublicationReport, REPORT_ENV, REPORT_FILE, ReportConfig, Reporting, Stage, WAIT_STEM,
    WAIT_WARN_THRESHOLD, wait_marker, write,
};
use super::fragment_tests::TempRoot;
use super::lock_support::publish_fixture;

/// Builds a report for one stage and outcome.
fn report(stage: Stage, outcome: Outcome) -> PublicationReport<'static> {
    PublicationReport {
        crate_name: "fixture",
        stage,
        outcome,
        lock_wait: Duration::from_millis(3),
        elapsed: Duration::from_millis(9),
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

    let raw = fs::read_to_string(root.path().join(REPORT_FILE)).context("read the build report")?;
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

/// Verifies every stage has a distinct serialized name.
///
/// A reader that has to interpret the report mechanically cannot afford two
/// stages collapsing to one token, which is exactly what a hand-written
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

/// Verifies neither channel file is mistaken for rendered inventory.
///
/// Pruning deletes rendered files the current render does not produce, so a
/// channel filename that matched the artefact matcher would be deleted on the
/// next publication and the report would silently vanish. Both names therefore
/// have to sit outside that matcher.
#[test]
fn channel_files_are_not_rendered_artefacts() -> Result<()> {
    ensure!(
        !super::is_rendered_artefact(REPORT_FILE),
        "the build report must not be pruned as superseded inventory",
    );
    let wait_name = format!("{WAIT_STEM}.1234.json");
    ensure!(
        !super::is_rendered_artefact(&wait_name),
        "a wait marker must not be pruned as superseded inventory",
    );
    ensure!(
        !super::is_rendered_artefact(WAIT_STEM),
        "the bare wait stem must not be treated as inventory either",
    );
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
        !root.path().join(REPORT_FILE).exists(),
        "a quiet build must not write a build report",
    );
    ensure!(
        !wait_marker(root.path()).exists(),
        "a quiet build must not write a wait marker",
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

    let raw = fs::read_to_string(root.path().join(REPORT_FILE)).context("read the build report")?;
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
    let temporary = root.path().join(REPORT_FILE).with_extension("tmp");
    ensure!(
        !temporary.exists(),
        "the temporary used for the atomic write must not be left behind",
    );
    let raw = fs::read_to_string(root.path().join(REPORT_FILE)).context("read the build report")?;
    serde_json::from_str::<serde_json::Value>(&raw).context("the report must be whole JSON")?;
    Ok(())
}

/// Verifies the channel's opt-in name is the documented one.
///
/// The name is the channel's whole interface for a build maintainer, so a
/// rename that no test noticed would silently make the documented variable do
/// nothing.
#[test]
fn the_opt_in_variable_is_the_documented_name() -> Result<()> {
    ensure!(REPORT_ENV == "ORTHO_CONFIG_ARTEFACT_REPORT");
    ensure!(REPORT_FILE == "cli-identifiers.build-report.json");
    ensure!(WAIT_STEM == "cli-identifiers.lock-wait");
    Ok(())
}
