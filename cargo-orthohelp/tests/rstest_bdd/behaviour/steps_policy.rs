//! Agent-native policy check step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements the `when`/`then` steps that exercise `--check-agent-native`
//! against the nested fixture and assert the policy report that lands on
//! stdout. The nested fixture contains a destructive `admin prune` command
//! that declares no bypass flag, so deny mode must fail with exit code 3 and
//! warn mode must report findings without failing.

use std::io::Read;

use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp};

const NESTED_FIXTURE_ARGS: [&str; 6] = [
    "--package",
    "orthohelp_fixture",
    "--root-type",
    "orthohelp_fixture::NestedFixtureConfig",
    "--locale",
    "en-US",
];

#[when("I run cargo orthohelp with --check-agent-native=deny")]
fn run_check_deny(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_policy_check(orthohelp_context, &["--check-agent-native=deny"])
}

#[when("I run cargo orthohelp with --check-agent-native=warn")]
fn run_check_warn(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_policy_check(orthohelp_context, &["--check-agent-native=warn"])
}

#[when("I run cargo orthohelp with format agent-context and --check-agent-native=warn")]
fn run_check_warn_composed(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let mut args = Vec::from(["--format", "agent-context"]);
    args.push("--check-agent-native=warn");
    run_policy_check(orthohelp_context, &args)
}

#[when("I run cargo orthohelp with format agent-context and --check-agent-native=deny")]
fn run_check_deny_composed(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let mut args = Vec::from(["--format", "agent-context"]);
    args.push("--check-agent-native=deny");
    run_policy_check(orthohelp_context, &args)
}

fn run_policy_check(orthohelp_context: &mut OrthoHelpContext, extra: &[&str]) -> StepResult<()> {
    let mut args = Vec::from(NESTED_FIXTURE_ARGS);
    args.extend(extra.iter().copied());
    let output = run_orthohelp(orthohelp_context, &args)?;
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[then("the process exit code is 3")]
fn assert_exit_code_3(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_exit_code_is(orthohelp_context, 3)
}

#[then("the process exit code is 0")]
fn assert_exit_code_0(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_exit_code_is(orthohelp_context, 0)
}

fn assert_exit_code_is(orthohelp_context: &mut OrthoHelpContext, expected: i32) -> StepResult<()> {
    let output = orthohelp_context
        .last_output
        .with_ref(Clone::clone)
        .ok_or("last_output should be set")?;
    // Pair every exit-code assertion with a well-formed-report assertion so a
    // crash exiting with a different code cannot false-pass.
    parse_policy_report(&output)?.ok_or("policy report on stdout should be valid JSON")?;
    let actual = output
        .status
        .code()
        .ok_or("process should exit with a code")?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "exit code should be {expected}, got {actual}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

#[then("the policy report on stdout contains code {expected:string}")]
fn report_contains_code(
    orthohelp_context: &mut OrthoHelpContext,
    expected: String,
) -> StepResult<()> {
    let output = orthohelp_context
        .last_output
        .with_ref(Clone::clone)
        .ok_or("last_output should be set")?;
    let report =
        parse_policy_report(&output)?.ok_or("policy report on stdout should be valid JSON")?;
    let mut codes = report
        .get("results")
        .and_then(Value::as_array)
        .ok_or("policy report should have a results array")?
        .iter()
        .filter_map(|result| result.get("code").and_then(Value::as_str));
    if codes.any(|code| code == expected) {
        Ok(())
    } else {
        Err(format!("policy report does not contain code {expected}").into())
    }
}

#[then("the policy report on stdout is valid JSON")]
fn report_is_valid_json(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = orthohelp_context
        .last_output
        .with_ref(Clone::clone)
        .ok_or("last_output should be set")?;
    match parse_policy_report(&output)? {
        Some(_) => Ok(()),
        None => Err("policy report on stdout should be valid JSON".into()),
    }
}

#[then("the agent context file is written to the output directory")]
fn agent_context_file_written(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open("agent-context.json")?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    let value: Value = serde_json::from_str(&buffer)?;
    if value.get("commands").is_some() {
        Ok(())
    } else {
        Err("agent-context.json should contain a commands array".into())
    }
}

/// Parses the policy report from the last command output's stdout.
///
/// The report is the single JSON document written to stdout; a blank line and
/// any other stdout bytes are tolerated by scanning for the document's first
/// and last braces.
fn parse_policy_report(output: &std::process::Output) -> StepResult<Option<Value>> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let start = trimmed
        .find('{')
        .ok_or("stdout should contain a JSON object")?;
    let end = trimmed
        .rfind('}')
        .ok_or("stdout should contain a JSON object")?;
    let document = trimmed
        .get(start..=end)
        .ok_or_else(|| "stdout should contain a well-formed JSON object".to_owned())?;
    let value: Value = serde_json::from_str(document)
        .map_err(|err| format!("policy report is not valid JSON: {err}"))?;
    let version = value.get("version").and_then(Value::as_str).unwrap_or("");
    if version != "1" {
        return Err(format!("policy report schema version should be 1, got {version:?}").into());
    }
    Ok(Some(value))
}
