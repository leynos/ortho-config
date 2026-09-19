//! Tests for the policy check orchestration helpers.

use super::*;
use camino::Utf8Path;
use rstest::rstest;
use serde_json::json;

use crate::policy::{PolicyMode, PolicyReport, PolicySummary};

fn report_with(mode: PolicyMode, warn: usize, deny: usize) -> PolicyReport {
    let report = PolicyReport::empty(mode);
    PolicyReport {
        summary: PolicySummary {
            warn,
            deny,
            ..report.summary
        },
        ..report
    }
}

#[rstest]
fn policy_table_detected_when_present() {
    let metadata = json!({
        "ortho_config": { "policy": { "mode": "warn" } }
    });

    assert!(policy_table_in_metadata(&metadata));
}

#[rstest]
fn policy_table_absent_when_missing() {
    let metadata = json!({ "root_type": "demo::Config" });

    assert!(!policy_table_in_metadata(&metadata));
}

#[rstest]
#[case::absent_policy_table(false, "no [package.metadata.ortho_config.policy] table found")]
#[case::configured_policy_table(true, "configured or overridden to off")]
fn off_summary_explains_why_nothing_was_checked(
    #[case] table_found: bool,
    #[case] expected_reason: &str,
) {
    let report = report_with(PolicyMode::Off, 0, 0);
    let line = summary_line(
        &report,
        Utf8Path::new("out/policy-report.json"),
        table_found,
        "demo",
    );

    assert!(line.contains("policy mode off"));
    assert!(line.contains(expected_reason));
    assert!(line.contains("out/policy-report.json"));
}

#[rstest]
fn warn_summary_reports_severity_counts() {
    let report = report_with(PolicyMode::Warn, 1, 0);
    let line = summary_line(
        &report,
        Utf8Path::new("out/policy-report.json"),
        true,
        "demo",
    );

    assert!(line.contains("mode: warn"));
    assert!(line.contains("findings: 1 warn, 0 deny"));
    assert!(line.contains("demo"));
}
