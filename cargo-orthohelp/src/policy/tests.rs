//! Tests for the `cargo-orthohelp` policy report contract.

use super::*;
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
fn empty_report_uses_tool_owned_schema_version() {
    let report = PolicyReport::empty(PolicyMode::Warn);

    assert_eq!(report.version, ORTHO_POLICY_REPORT_SCHEMA_VERSION);
    assert_eq!(report.tool, "cargo-orthohelp");
    assert_eq!(report.mode, PolicyMode::Warn);
    assert!(report.results.is_empty());
    assert_eq!(report.summary, PolicySummary::default());
}

#[rstest]
fn report_serializes_stable_machine_fields() {
    let result = PolicyResult {
        rule_id: "agent-native.vocabulary.canonical-flag".to_owned(),
        code: "canonical_flag_missing".to_owned(),
        severity: PolicySeverity::Warn,
        message: "Use --json for structured output.".to_owned(),
        location: Some(SourceLocation {
            file: "Cargo.toml".to_owned(),
            range: Some(SourceRange {
                start: SourcePosition {
                    line: 12,
                    column: 1,
                },
                end: SourcePosition {
                    line: 12,
                    column: 20,
                },
            }),
        }),
    };
    let report = PolicyReport::with_results(PolicyMode::Warn, vec![result]);

    let value = serde_json::to_value(report).expect("serialize policy report");
    assert_eq!(field(&value, "version"), "1");
    assert_eq!(field(&value, "tool"), "cargo-orthohelp");
    assert_eq!(field(&value, "mode"), "warn");
    let serialized_result = first_array_item(field(&value, "results"));
    assert_eq!(
        field(serialized_result, "rule_id"),
        "agent-native.vocabulary.canonical-flag"
    );
    assert_eq!(field(serialized_result, "code"), "canonical_flag_missing");
    assert_eq!(field(serialized_result, "severity"), "warn");
    let summary = field(&value, "summary");
    assert_eq!(field(summary, "warn"), 1);
    assert_eq!(field(summary, "total"), 1);
}

#[rstest]
#[case(PolicySeverity::Off, (1, 0, 0, 1))]
#[case(PolicySeverity::Warn, (0, 1, 0, 1))]
#[case(PolicySeverity::Deny, (0, 0, 1, 1))]
fn summary_counts_severity_without_parsing_messages(
    #[case] severity: PolicySeverity,
    #[case] expected: (usize, usize, usize, usize),
) {
    let result = PolicyResult {
        rule_id: "agent-native.example".to_owned(),
        code: "example".to_owned(),
        severity,
        message: "message text is not a contract".to_owned(),
        location: None,
    };

    let summary = PolicySummary::from_results(&[result]);
    assert_eq!(
        (summary.off, summary.warn, summary.deny, summary.total),
        expected
    );
}

#[rstest]
#[case(json!({"tool": "cargo-orthohelp", "mode": "warn", "results": []}))]
#[case(json!({"version": "1", "mode": "warn", "results": []}))]
#[case(json!({"version": "1", "tool": "cargo-orthohelp", "results": []}))]
#[case(json!({"version": "1", "tool": "cargo-orthohelp", "mode": "warn"}))]
fn missing_required_report_fields_fail_deserialization(#[case] payload: Value) {
    let error = serde_json::from_value::<PolicyReport>(payload)
        .expect_err("missing required report fields should fail");

    assert!(
        error.is_data() || error.is_syntax(),
        "expected a data or syntax error, got {error}"
    );
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value
        .get(name)
        .unwrap_or_else(|| panic!("JSON object should contain `{name}`"))
}

fn first_array_item(value: &Value) -> &Value {
    value
        .as_array()
        .and_then(|items| items.first())
        .expect("JSON value should be a non-empty array")
}
