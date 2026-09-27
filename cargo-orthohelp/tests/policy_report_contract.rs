//! Integration contracts for emitted agent-native policy-report JSON.
//!
//! These tests invoke the compiled binary and validate the report artefact
//! independently from unit-level report construction and snapshots.

mod fixtures;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::rstest;
use serde_json::Value;
use std::error::Error;
use std::process::{Command, Output};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const EXPECTED_SCHEMA_VERSION: &str = "1";
const WARN_RULE_ID: &str = "agent-native.config.redundant-exception";
const WARN_CODE: &str = "redundant_exception";
const DENY_RULE_ID: &str = "agent-native.config.malformed-exception";
const DENY_CODE: &str = "malformed_exception";

#[derive(Debug, Clone, Copy)]
struct PolicyCase {
    mode: &'static str,
    package: &'static str,
    should_succeed: bool,
    finding: Option<Finding>,
    summary: SummaryCounts,
}

#[derive(Debug, Clone, Copy)]
struct Finding {
    rule_id: &'static str,
    code: &'static str,
    severity: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct SummaryCounts {
    off: usize,
    warn: usize,
    deny: usize,
    total: usize,
}

#[rstest]
#[case::warn_clean(PolicyCase {
    mode: "warn",
    package: "orthohelp_fixture",
    should_succeed: true,
    finding: None,
    summary: SummaryCounts { off: 0, warn: 0, deny: 0, total: 0 },
})]
#[case::warn_finding(PolicyCase {
    mode: "warn",
    package: "orthohelp_policy_warn_fixture",
    should_succeed: true,
    finding: Some(Finding { rule_id: WARN_RULE_ID, code: WARN_CODE, severity: "warn" }),
    summary: SummaryCounts { off: 0, warn: 1, deny: 0, total: 1 },
})]
#[case::deny_clean(PolicyCase {
    mode: "deny",
    package: "orthohelp_fixture",
    should_succeed: true,
    finding: None,
    summary: SummaryCounts { off: 0, warn: 0, deny: 0, total: 0 },
})]
#[case::deny_finding(PolicyCase {
    mode: "deny",
    package: "orthohelp_policy_deny_fixture",
    should_succeed: false,
    finding: Some(Finding { rule_id: DENY_RULE_ID, code: DENY_CODE, severity: "deny" }),
    summary: SummaryCounts { off: 0, warn: 0, deny: 1, total: 1 },
})]
#[case::off_suppresses_findings(PolicyCase {
    mode: "off",
    package: "orthohelp_policy_off_fixture",
    should_succeed: true,
    finding: None,
    summary: SummaryCounts { off: 0, warn: 0, deny: 0, total: 0 },
})]
fn emitted_policy_report_has_stable_contract(#[case] case: PolicyCase) -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_policy_check(&out_dir, case)?;

    assert_exit_status(&output, case)?;
    let report = read_policy_report(&out_dir)?;
    assert_string_field(&report, "version", EXPECTED_SCHEMA_VERSION)?;
    assert_string_field(&report, "tool", "cargo-orthohelp")?;
    assert_string_field(&report, "mode", case.mode)?;
    assert_results(&report, case.finding)?;
    assert_summary(&report, case.summary)
}

fn run_policy_check(out_dir: &TempDir, case: PolicyCase) -> TestResult<Output> {
    let executable = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(executable.as_str());
    // Policy-only runs return before the generator and bridge-build path.
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .args([
            "orthohelp",
            "--check-agent-native",
            "--policy-mode",
            case.mode,
            "--package",
            case.package,
            "--out-dir",
        ])
        .arg(out_dir.path());
    Ok(command.output()?)
}

fn assert_exit_status(output: &Output, case: PolicyCase) -> TestResult {
    if output.status.success() != case.should_succeed {
        return Err(format!(
            "unexpected process status {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    if case
        .finding
        .is_some_and(|finding| finding.severity == "deny")
        && !String::from_utf8_lossy(&output.stderr).contains("PolicyViolation")
    {
        return Err("deny finding should produce a policy validation failure".into());
    }
    Ok(())
}

fn read_policy_report(out_dir: &TempDir) -> TestResult<Value> {
    let out_path = Utf8PathBuf::from_path_buf(out_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    let dir = Dir::open_ambient_dir(&out_path, ambient_authority())?;
    let serialized = dir.read_to_string("policy-report.json")?;
    Ok(serde_json::from_str(&serialized)?)
}

fn assert_results(report: &Value, expected: Option<Finding>) -> TestResult {
    let results = report
        .get("results")
        .and_then(Value::as_array)
        .ok_or("results should be an array")?;
    match expected {
        Some(finding) => {
            if results.len() != 1 {
                return Err(format!("results should contain one finding, got {results:?}").into());
            }
            let result = results.first().ok_or("finding should be present")?;
            assert_string_field(result, "rule_id", finding.rule_id)?;
            assert_string_field(result, "code", finding.code)?;
            assert_string_field(result, "severity", finding.severity)?;
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

fn assert_summary(report: &Value, expected: SummaryCounts) -> TestResult {
    let summary = report
        .get("summary")
        .and_then(Value::as_object)
        .ok_or("summary should be an object")?;
    for (field, expected_count) in [
        ("off", expected.off),
        ("warn", expected.warn),
        ("deny", expected.deny),
        ("total", expected.total),
    ] {
        let raw_count = summary
            .get(field)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("summary {field} should be an unsigned number"))?;
        let actual_count = usize::try_from(raw_count)?;
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
