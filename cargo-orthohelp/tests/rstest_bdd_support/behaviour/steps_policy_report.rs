//! Policy-report JSON step definitions for `cargo-orthohelp` behavioural tests.
//!
//! The scenarios invoke the compiled binary, parse its stdout, and assert the
//! stable policy-report contract across its enforcement modes.

use std::fmt;

use cargo_orthohelp::policy::ORTHO_POLICY_REPORT_SCHEMA_VERSION;
use rstest_bdd_macros::{then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepError, StepResult, run_orthohelp};

const CANONICAL_FLAG_RULE_ID: &str = "agent-native.vocabulary.canonical-flag";
const NON_CANONICAL_FLAG_CODE: &str = "non_canonical_flag";

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

#[when("I run cargo-orthohelp policy check in warn mode for the simple fixture")]
fn run_warn_policy_check_for_simple_fixture(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    run_policy_check(
        orthohelp_context,
        "warn",
        Some("orthohelp_fixture::SimpleFixtureConfig"),
    )
}

#[when("I run cargo-orthohelp policy check in warn mode for the fixture")]
fn run_warn_policy_check_for_fixture(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_policy_check(orthohelp_context, "warn", None)
}

#[when("I run cargo-orthohelp policy check in deny mode for the fixture")]
fn run_deny_policy_check_for_fixture(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_policy_check(orthohelp_context, "deny", None)
}

#[when("I run cargo-orthohelp policy check in off mode for the fixture")]
fn run_off_policy_check_for_fixture(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_policy_check(orthohelp_context, "off", None)
}

fn run_policy_check(
    ctx: &mut OrthoHelpContext,
    mode: &str,
    root_type: Option<&str>,
) -> StepResult<()> {
    let mut args = vec![
        "--package",
        "orthohelp_fixture",
        "--check-agent-native",
        "--policy-mode",
        mode,
    ];
    if let Some(selected_root_type) = root_type {
        args.extend(["--root-type", selected_root_type]);
    }
    ctx.last_output.set(run_orthohelp(ctx, &args)?);
    Ok(())
}

#[then("the policy report has warn mode and no findings")]
fn policy_report_has_empty_warn_results(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    assert_empty_report(orthohelp_context, "warn")
}

#[then("the policy report has off mode and no findings")]
fn policy_report_has_empty_off_results(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_empty_report(orthohelp_context, "off")
}

#[then("the policy report has one warning finding")]
fn policy_report_has_warning_finding(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_finding_report(orthohelp_context, "warn", "warn", true)
}

#[then("the policy report has one deny finding and a validation failure")]
fn policy_report_has_deny_finding(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_finding_report(orthohelp_context, "deny", "deny", false)
}

fn assert_empty_report(ctx: &OrthoHelpContext, expected_mode: &str) -> StepResult<()> {
    let run = policy_run(ctx)?;
    if !run.is_success {
        return Err(format!("policy check should succeed: {}", run.stderr).into());
    }
    assert_report_header(&run.report, expected_mode)?;
    expect_empty_results(&run.report)?;
    expect_summary(&run.report, (0, 0, 0, 0))
}

fn assert_finding_report(
    ctx: &OrthoHelpContext,
    expected_mode: &str,
    expected_severity: &str,
    should_succeed: bool,
) -> StepResult<()> {
    let run = policy_run(ctx)?;
    if run.is_success != should_succeed {
        return Err(format!("unexpected policy check status: {}", run.stderr).into());
    }
    if !should_succeed && !run.stderr.contains("AgentNativePolicyDenied") {
        return Err("deny policy check should report a validation failure".into());
    }
    assert_report_header(&run.report, expected_mode)?;
    expect_one_finding(&run.report, expected_severity)?;
    let summary = match expected_severity {
        "warn" => (0, 1, 0, 1),
        "deny" => (0, 0, 1, 1),
        _ => return Err(format!("unsupported severity {expected_severity}").into()),
    };
    expect_summary(&run.report, summary)
}

struct PolicyRun {
    is_success: bool,
    stderr: String,
    report: Value,
}

fn policy_run(ctx: &OrthoHelpContext) -> StepResult<PolicyRun> {
    ctx.last_output
        .with_ref(|output| -> StepResult<PolicyRun> {
            if !output.stdout.ends_with(b"\n") {
                return Err("policy report should have a trailing newline".into());
            }
            Ok(PolicyRun {
                is_success: output.status.success(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                report: serde_json::from_slice(&output.stdout)?,
            })
        })
        .ok_or_else(|| -> StepError { "last_output should be set".into() })?
}

fn assert_report_header(report: &Value, expected_mode: &str) -> StepResult<()> {
    expect_string_field(
        report,
        JsonField::Version,
        ORTHO_POLICY_REPORT_SCHEMA_VERSION,
    )?;
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

fn expect_one_finding(report: &Value, expected_severity: &str) -> StepResult<()> {
    let results = report
        .get(JsonField::Results.as_str())
        .and_then(Value::as_array)
        .ok_or("results should be an array")?;
    if results.len() != 1 {
        return Err(format!("results should contain one finding, got {results:?}").into());
    }
    let result = results.first().ok_or("finding should exist")?;
    expect_string_field(result, JsonField::RuleId, CANONICAL_FLAG_RULE_ID)?;
    expect_string_field(result, JsonField::Code, NON_CANONICAL_FLAG_CODE)?;
    expect_string_field(result, JsonField::Severity, expected_severity)?;
    let message = string_field(result, JsonField::Message)?;
    if message.is_empty() {
        return Err("finding message should not be empty".into());
    }
    if result.get(JsonField::Location.as_str()).is_none() {
        return Err("finding should contain location".into());
    }
    Ok(())
}

fn expect_summary(report: &Value, expected: (usize, usize, usize, usize)) -> StepResult<()> {
    let summary = report
        .get(JsonField::Summary.as_str())
        .ok_or("summary should be present")?;
    for (field, expected_count) in [
        (JsonField::Off, expected.0),
        (JsonField::Warn, expected.1),
        (JsonField::Deny, expected.2),
        (JsonField::Total, expected.3),
    ] {
        let actual = summary
            .get(field.as_str())
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{field} should be an unsigned number"))?;
        let actual = usize::try_from(actual)?;
        if actual != expected_count {
            return Err(format!("{field} should be {expected_count}, got {actual}").into());
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
