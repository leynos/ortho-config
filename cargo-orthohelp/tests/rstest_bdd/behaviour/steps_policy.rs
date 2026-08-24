//! Agent-native policy check step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements the `given`/`when`/`then` steps that exercise the
//! `--check-agent-native` command surface against the policy fixture packages:
//!
//! - **`given`** steps select the target package by name: the warn fixture, the
//!   deny fixture, or a package with no policy table (`orthohelp_fixture`).
//! - **`when`** steps run `cargo-orthohelp --check-agent-native` against the
//!   selected package, optionally with a `--policy-mode` override, and record
//!   the subprocess output.
//! - **`then`** steps assert the exit status, the `policy-report.json` contents
//!   (mode, findings, exceptions, vocabulary, summary), and the stderr summary.
//!

use std::io::Read;

use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{given, then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp};

/// Package name fixtures used by the policy scenarios.
const WARN_FIXTURE: &str = "orthohelp_policy_warn_fixture";
const DENY_FIXTURE: &str = "orthohelp_policy_deny_fixture";
const OFF_FIXTURE: &str = "orthohelp_policy_off_fixture";
const NO_POLICY_FIXTURE: &str = "orthohelp_fixture";

#[given("the policy warn fixture package")]
fn policy_warn_fixture_package(orthohelp_context: &mut OrthoHelpContext) {
    orthohelp_context
        .policy_package
        .set(WARN_FIXTURE.to_owned());
}

#[given("the policy deny fixture package")]
fn policy_deny_fixture_package(orthohelp_context: &mut OrthoHelpContext) {
    orthohelp_context
        .policy_package
        .set(DENY_FIXTURE.to_owned());
}

#[given("the policy off fixture package")]
fn policy_off_fixture_package(orthohelp_context: &mut OrthoHelpContext) {
    orthohelp_context.policy_package.set(OFF_FIXTURE.to_owned());
}

#[given("a fixture package with no policy table")]
fn no_policy_fixture_package(orthohelp_context: &mut OrthoHelpContext) {
    orthohelp_context
        .policy_package
        .set(NO_POLICY_FIXTURE.to_owned());
}

#[when("cargo orthohelp runs with --check-agent-native")]
fn run_policy_check(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_policy_check_args(orthohelp_context, &[])
}

#[when("cargo orthohelp runs with --check-agent-native --policy-mode {mode}")]
fn run_policy_check_override(
    orthohelp_context: &mut OrthoHelpContext,
    mode: String,
) -> StepResult<()> {
    run_policy_check_args(orthohelp_context, &["--policy-mode", &mode])
}

fn run_policy_check_args(
    orthohelp_context: &mut OrthoHelpContext,
    extra_args: &[&str],
) -> StepResult<()> {
    let package = orthohelp_context
        .policy_package
        .with_ref(Clone::clone)
        .ok_or("policy package should be set")?;
    let mut args = vec!["--check-agent-native", "--package", package.as_str()];
    args.extend_from_slice(extra_args);
    let output = run_orthohelp(orthohelp_context, &args)?;
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[then("the command succeeds")]
fn command_succeeds(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let succeeded = orthohelp_context
        .last_output
        .with_ref(|output| output.status.success())
        .ok_or("last_output should be set")?;
    if succeeded {
        Ok(())
    } else {
        let stderr_bytes = orthohelp_context
            .last_output
            .with_ref(|output| output.stderr.clone())
            .ok_or("last_output should be set")?;
        let stderr = String::from_utf8_lossy(&stderr_bytes);
        Err(format!("cargo-orthohelp should succeed: {stderr}").into())
    }
}

#[then("the command fails with a policy violation")]
fn command_fails_with_policy_violation(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let failed_with_violation = orthohelp_context
        .last_output
        .with_ref(|output| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            !output.status.success() && stderr.contains("PolicyViolation")
        })
        .ok_or("last_output should be set")?;
    if failed_with_violation {
        Ok(())
    } else {
        Err("expected a deny-mode policy violation".into())
    }
}

#[then("the policy report lists one warning with code {code}")]
fn policy_report_lists_one_warning(
    orthohelp_context: &mut OrthoHelpContext,
    code: String,
) -> StepResult<()> {
    let report = read_policy_report(orthohelp_context)?;
    let warning_codes = report
        .get("results")
        .and_then(Value::as_array)
        .ok_or("results field missing")?
        .iter()
        .filter(|result| result.get("severity").and_then(Value::as_str) == Some("warn"))
        .filter_map(|result| result.get("code").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        warning_codes,
        vec![code.as_str()],
        "expected exactly one warning with the given code"
    );
    Ok(())
}

#[then("the policy report lists the configured exceptions")]
fn policy_report_lists_configured_exceptions(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let report = read_policy_report(orthohelp_context)?;
    let exceptions = report
        .get("exceptions")
        .and_then(Value::as_array)
        .ok_or("exceptions field missing")?;
    assert_eq!(exceptions.len(), 2, "warn fixture has two exceptions");
    let first = exceptions.first().ok_or("first exception missing")?;
    assert_eq!(string_field(first, "kind")?, "verb");
    assert_eq!(string_field(first, "name")?, "get");
    assert_eq!(
        string_field(first, "reason")?,
        "redundant but part of the migration surface"
    );
    let second = exceptions.get(1).ok_or("second exception missing")?;
    assert_eq!(string_field(second, "kind")?, "flag");
    assert_eq!(string_field(second, "name")?, "--format");
    assert_eq!(string_field(second, "reason")?, "scoped legacy flag");
    assert_eq!(string_field(second, "command_path")?, "fixture");
    Ok(())
}

#[then("the policy report lists the canonical vocabulary")]
fn policy_report_lists_canonical_vocabulary(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let report = read_policy_report(orthohelp_context)?;
    let vocabulary = report.get("vocabulary").ok_or("vocabulary field missing")?;
    let verbs = string_array_field(vocabulary, "verbs")?;
    let flags = string_array_field(vocabulary, "flags")?;
    assert!(
        verbs.contains(&"get".to_owned()),
        "verbs should include 'get'"
    );
    assert!(
        flags.contains(&"--json".to_owned()),
        "flags should include '--json'"
    );
    Ok(())
}

#[then("the policy report summary counts one deny finding")]
fn policy_report_summary_counts_one_deny(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let report = read_policy_report(orthohelp_context)?;
    let summary = report.get("summary").ok_or("summary field missing")?;
    let deny = summary
        .get("deny")
        .and_then(Value::as_u64)
        .ok_or("summary.deny missing")?;
    assert_eq!(deny, 1, "expected one deny finding");
    Ok(())
}

#[then("the policy report records mode {mode} and no findings")]
fn policy_report_records_mode_no_findings(
    orthohelp_context: &mut OrthoHelpContext,
    mode: String,
) -> StepResult<()> {
    let report = read_policy_report(orthohelp_context)?;
    assert_eq!(string_field(&report, "mode")?, mode.as_str());
    let results = report
        .get("results")
        .and_then(Value::as_array)
        .ok_or("results field missing")?;
    assert!(results.is_empty(), "expected no findings");
    Ok(())
}

#[then("the policy report records mode {mode}")]
fn policy_report_records_mode(
    orthohelp_context: &mut OrthoHelpContext,
    mode: String,
) -> StepResult<()> {
    let report = read_policy_report(orthohelp_context)?;
    assert_eq!(string_field(&report, "mode")?, mode.as_str());
    Ok(())
}

#[then("standard error notes that nothing was checked")]
fn stderr_notes_nothing_checked(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let stderr_bytes = orthohelp_context
        .last_output
        .with_ref(|output| output.stderr.clone())
        .ok_or("last_output should be set")?;
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    assert!(
        stderr.contains("nothing was checked"),
        "stderr should note that nothing was checked: {stderr}"
    );
    Ok(())
}

fn read_policy_report(orthohelp_context: &mut OrthoHelpContext) -> StepResult<Value> {
    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open("policy-report.json")?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    Ok(serde_json::from_str(&buffer)?)
}

fn string_field<'a>(value: &'a Value, field: &str) -> StepResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} field missing").into())
}

fn string_array_field(value: &Value, field: &str) -> StepResult<Vec<String>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{field} field missing"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{field} item should be a string").into())
        })
        .collect()
}
