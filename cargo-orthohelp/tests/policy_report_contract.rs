//! Integration contracts for emitted agent-native policy-report JSON.
//!
//! These tests exercise the compiled `cargo-orthohelp` binary and assert the
//! stable JSON contract independently from unit-level report construction.

mod fixtures;

use cargo_orthohelp::policy::ORTHO_POLICY_REPORT_SCHEMA_VERSION;
use rstest::rstest;
use serde_json::Value;
use std::error::Error;
use std::process::{Command, Output};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const CANONICAL_FLAG_RULE_ID: &str = "agent-native.vocabulary.canonical-flag";
const NON_CANONICAL_FLAG_CODE: &str = "non_canonical_flag";

#[derive(Debug, Clone, Copy)]
struct PolicyReportCase {
    mode: &'static str,
    root_type: Option<&'static str>,
    should_succeed: bool,
    expected_severity: Option<&'static str>,
    expected_summary: (usize, usize, usize, usize),
}

#[rstest]
#[case::warn_clean(PolicyReportCase {
    mode: "warn",
    root_type: Some("orthohelp_fixture::SimpleFixtureConfig"),
    should_succeed: true,
    expected_severity: None,
    expected_summary: (0, 0, 0, 0),
})]
#[case::warn_finding(PolicyReportCase {
    mode: "warn",
    root_type: None,
    should_succeed: true,
    expected_severity: Some("warn"),
    expected_summary: (0, 1, 0, 1),
})]
#[case::deny_finding(PolicyReportCase {
    mode: "deny",
    root_type: None,
    should_succeed: false,
    expected_severity: Some("deny"),
    expected_summary: (0, 0, 1, 1),
})]
#[case::off_suppresses_finding(PolicyReportCase {
    mode: "off",
    root_type: None,
    should_succeed: true,
    expected_severity: None,
    expected_summary: (0, 0, 0, 0),
})]
fn emitted_policy_report_has_stable_contract(#[case] case: PolicyReportCase) -> TestResult {
    let output = run_policy_check(case.mode, case.root_type)?;

    assert_exit_status(&output, case.should_succeed)?;
    if !output.stdout.ends_with(b"\n") {
        return Err("policy report should have a trailing newline".into());
    }
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_string_field(&report, "version", ORTHO_POLICY_REPORT_SCHEMA_VERSION)?;
    assert_string_field(&report, "tool", "cargo-orthohelp")?;
    assert_string_field(&report, "mode", case.mode)?;
    assert_results(&report, case.expected_severity)?;
    assert_summary(&report, case.expected_summary)
}

fn run_policy_check(mode: &str, root_type: Option<&str>) -> TestResult<Output> {
    let executable = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(executable.as_str());
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .args([
            "orthohelp",
            "--package",
            "orthohelp_fixture",
            "--check-agent-native",
            "--policy-mode",
            mode,
        ]);
    if let Some(selected_root_type) = root_type {
        command.args(["--root-type", selected_root_type]);
    }
    Ok(command.output()?)
}

fn assert_exit_status(output: &Output, should_succeed: bool) -> TestResult {
    if output.status.success() == should_succeed {
        return Ok(());
    }
    Err(format!(
        "unexpected process status {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn assert_results(report: &Value, expected_severity: Option<&str>) -> TestResult {
    let results = report
        .get("results")
        .and_then(Value::as_array)
        .ok_or("results should be an array")?;
    match expected_severity {
        Some(severity) => {
            let result = results.first().ok_or("results should not be empty")?;
            assert_string_field(result, "rule_id", CANONICAL_FLAG_RULE_ID)?;
            assert_string_field(result, "code", NON_CANONICAL_FLAG_CODE)?;
            assert_string_field(result, "severity", severity)?;
            let message = result
                .get("message")
                .and_then(Value::as_str)
                .ok_or("result message should be a string")?;
            if message.is_empty() {
                return Err("result message should not be empty".into());
            }
            if result.get("location").is_none() {
                return Err("result should contain location".into());
            }
            Ok(())
        }
        None if results.is_empty() => Ok(()),
        None => Err(format!("results should be empty, got {results:?}").into()),
    }
}

fn assert_summary(report: &Value, expected: (usize, usize, usize, usize)) -> TestResult {
    let summary = report
        .get("summary")
        .and_then(Value::as_object)
        .ok_or("summary should be an object")?;
    for (field, expected_count) in [
        ("off", expected.0),
        ("warn", expected.1),
        ("deny", expected.2),
        ("total", expected.3),
    ] {
        let actual_json_count = summary
            .get(field)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("summary {field} should be an unsigned number"))?;
        let actual_count = usize::try_from(actual_json_count)?;
        if actual_count != expected_count {
            return Err(
                format!("summary {field} should be {expected_count}, got {actual_count}").into(),
            );
        }
    }
    Ok(())
}

fn assert_string_field(value: &Value, field: &str, expected: &str) -> TestResult {
    let actual = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} should be a string"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} should be {expected:?}, got {actual:?}").into())
    }
}
