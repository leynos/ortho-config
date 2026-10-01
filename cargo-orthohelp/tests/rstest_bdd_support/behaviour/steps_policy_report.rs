//! Policy-report JSON contract assertions for `cargo-orthohelp` scenarios.
//!
//! The shared policy steps invoke the compiled binary. These assertions
//! deserialize captured stdout and verify the stable schema and enforcement
//! summary.

use std::fmt;

use rstest_bdd_macros::then;
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult};

const EXPECTED_SCHEMA_VERSION: &str = "1";

const WARN_EXPECTATION: ExpectedFinding = ExpectedFinding {
    mode: "warn",
    rule_id: "agent-native.config.redundant-exception",
    code: "redundant_exception",
    severity: "warn",
    should_succeed: true,
    summary: SummaryCounts {
        off: 0,
        warn: 1,
        deny: 0,
        total: 1,
    },
};
const DENY_EXPECTATION: ExpectedFinding = ExpectedFinding {
    mode: "deny",
    rule_id: "agent-native.config.malformed-exception",
    code: "malformed_exception",
    severity: "deny",
    should_succeed: false,
    summary: SummaryCounts {
        off: 0,
        warn: 0,
        deny: 1,
        total: 1,
    },
};
const EMPTY_SUMMARY: SummaryCounts = SummaryCounts {
    off: 0,
    warn: 0,
    deny: 0,
    total: 0,
};

#[derive(Debug, Clone, Copy)]
struct ExpectedFinding {
    mode: &'static str,
    rule_id: &'static str,
    code: &'static str,
    severity: &'static str,
    should_succeed: bool,
    summary: SummaryCounts,
}

#[derive(Debug, Clone, Copy)]
struct SummaryCounts {
    off: usize,
    warn: usize,
    deny: usize,
    total: usize,
}

#[derive(Debug, Clone, Copy)]
enum JsonField {
    Version,
    Tool,
    Mode,
    Results,
    Summary,
    Off,
    Warn,
    Deny,
    Total,
    RuleId,
    Code,
    Severity,
    Message,
    Location,
}

impl JsonField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Tool => "tool",
            Self::Mode => "mode",
            Self::Results => "results",
            Self::Summary => "summary",
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Total => "total",
            Self::RuleId => "rule_id",
            Self::Code => "code",
            Self::Severity => "severity",
            Self::Message => "message",
            Self::Location => "location",
        }
    }
}

impl fmt::Display for JsonField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[then("stdout policy report has {mode} mode and no findings")]
fn policy_report_has_empty_results(
    orthohelp_context: &mut OrthoHelpContext,
    mode: String,
) -> StepResult<()> {
    let run = policy_run(orthohelp_context)?;
    if !run.is_success {
        return Err(format!("policy check should succeed: {}", run.stderr).into());
    }
    assert_report_header(&run.report, &mode)?;
    expect_empty_results(&run.report)?;
    expect_summary(&run.report, EMPTY_SUMMARY)
}

#[then("stdout contains a policy report with one warning finding")]
fn policy_report_has_warning_finding(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_finding_report(orthohelp_context, WARN_EXPECTATION)
}

#[then("stdout contains a policy report with one deny finding and a validation failure")]
fn policy_report_has_deny_finding(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_finding_report(orthohelp_context, DENY_EXPECTATION)
}

fn assert_finding_report(ctx: &OrthoHelpContext, expected: ExpectedFinding) -> StepResult<()> {
    let run = policy_run(ctx)?;
    if run.is_success != expected.should_succeed {
        return Err(format!("unexpected policy check status: {}", run.stderr).into());
    }
    if !expected.should_succeed && !run.stderr.contains("PolicyViolation") {
        return Err("deny policy check should report a validation failure".into());
    }
    assert_report_header(&run.report, expected.mode)?;
    expect_one_finding(&run.report, expected)?;
    expect_summary(&run.report, expected.summary)
}

struct PolicyRun {
    is_success: bool,
    stderr: String,
    report: Value,
}

fn policy_run(ctx: &OrthoHelpContext) -> StepResult<PolicyRun> {
    let (is_success, stderr, stdout) = ctx
        .last_output
        .with_ref(|output| {
            (
                output.status.success(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
                output.stdout.clone(),
            )
        })
        .ok_or("last_output should be set")?;
    assert_one_compact_json_line(&stdout)?;
    Ok(PolicyRun {
        is_success,
        stderr,
        report: serde_json::from_slice(&stdout)?,
    })
}

fn assert_one_compact_json_line(stdout: &[u8]) -> StepResult<()> {
    if stdout
        .strip_suffix(b"\n")
        .is_some_and(|document| !document.contains(&b'\n'))
    {
        Ok(())
    } else {
        Err("stdout should contain one compact JSON document and a trailing newline".into())
    }
}

fn assert_report_header(report: &Value, expected_mode: &str) -> StepResult<()> {
    expect_string_field(report, JsonField::Version, EXPECTED_SCHEMA_VERSION)?;
    expect_string_field(report, JsonField::Tool, "cargo-orthohelp")?;
    expect_string_field(report, JsonField::Mode, expected_mode)
}

fn expect_empty_results(report: &Value) -> StepResult<()> {
    match report
        .get(JsonField::Results.as_str())
        .and_then(Value::as_array)
    {
        Some(results) if results.is_empty() => Ok(()),
        Some(results) => Err(format!("results should be empty, got {results:?}").into()),
        None => Err("results should be an array".into()),
    }
}

fn expect_one_finding(report: &Value, expected: ExpectedFinding) -> StepResult<()> {
    let results = report
        .get(JsonField::Results.as_str())
        .and_then(Value::as_array)
        .ok_or("results should be an array")?;
    if results.len() != 1 {
        return Err(format!("results should contain one finding, got {results:?}").into());
    }
    let result = results.first().ok_or("finding should exist")?;
    expect_string_field(result, JsonField::RuleId, expected.rule_id)?;
    expect_string_field(result, JsonField::Code, expected.code)?;
    expect_string_field(result, JsonField::Severity, expected.severity)?;
    let message = string_field(result, JsonField::Message)?;
    if message.is_empty() {
        return Err("finding message should not be empty".into());
    }
    if result.get(JsonField::Location.as_str()).is_none() {
        return Err("finding should contain location".into());
    }
    Ok(())
}

fn expect_summary(report: &Value, expected: SummaryCounts) -> StepResult<()> {
    let summary = report
        .get(JsonField::Summary.as_str())
        .ok_or("summary should be present")?;
    for (field, expected_count) in [
        (JsonField::Off, expected.off),
        (JsonField::Warn, expected.warn),
        (JsonField::Deny, expected.deny),
        (JsonField::Total, expected.total),
    ] {
        let raw_count = summary
            .get(field.as_str())
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{field} should be an unsigned number"))?;
        let actual_count = usize::try_from(raw_count)?;
        if actual_count != expected_count {
            return Err(format!("{field} should be {expected_count}, got {actual_count}").into());
        }
    }
    Ok(())
}

fn expect_string_field(value: &Value, field: JsonField, expected: &str) -> StepResult<()> {
    let actual = string_field(value, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} should be {expected:?}, got {actual:?}").into())
    }
}

fn string_field(value: &Value, field: JsonField) -> StepResult<&str> {
    value
        .get(field.as_str())
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} should be a string").into())
}
