//! Agent-context JSON step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements the `when`/`then` steps that exercise `--format agent-context`
//! against the fixture crate and assert the compact JSON contract that agents
//! consume.

use ortho_config::AGENT_CONTEXT_KIND_SUFFIX;
use rstest_bdd_macros::{then, when};
use serde_json::Value;
use std::fmt;

use super::steps::{OrthoHelpContext, StepResult, run_orthohelp};

const EXPECTED_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Copy)]
pub(super) enum JsonField {
    SchemaVersion,
    Kind,
    Commands,
    Path,
    Summary,
    Inputs,
    Required,
    InteractionMode,
    MutationEffect,
    BypassFlag,
}

impl JsonField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaVersion => "schema_version",
            Self::Kind => "kind",
            Self::Commands => "commands",
            Self::Path => "path",
            Self::Summary => "summary",
            Self::Inputs => "inputs",
            Self::Required => "required",
            Self::InteractionMode => "interaction_mode",
            Self::MutationEffect => "mutation_effect",
            Self::BypassFlag => "bypass_flag",
        }
    }
}

impl fmt::Display for JsonField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

const BASE_AGENT_CONTEXT_ARGS: [&str; 4] = [
    "--format",
    "agent-context",
    "--package",
    "orthohelp_fixture",
];

#[when("I run cargo-orthohelp with format agent-context for the fixture")]
fn run_with_format_agent_context(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_with_agent_context_args(orthohelp_context, &BASE_AGENT_CONTEXT_ARGS)
}

#[when("I run cargo-orthohelp with format agent-context for the simple fixture")]
fn run_with_format_agent_context_simple(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let mut args = Vec::from(BASE_AGENT_CONTEXT_ARGS);
    args.extend(["--root-type", "orthohelp_fixture::SimpleFixtureConfig"]);
    run_with_agent_context_args(orthohelp_context, &args)
}

#[when("I run cargo-orthohelp with format agent-context for the nested fixture")]
fn run_with_format_agent_context_nested(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let mut args = Vec::from(BASE_AGENT_CONTEXT_ARGS);
    args.extend(["--root-type", "orthohelp_fixture::NestedFixtureConfig"]);
    run_with_agent_context_args(orthohelp_context, &args)
}

fn run_with_agent_context_args(
    orthohelp_context: &mut OrthoHelpContext,
    args: &[&str],
) -> StepResult<()> {
    let output = run_orthohelp(orthohelp_context, args)?;
    if !output.status.success() {
        return Err(format!(
            "cargo-orthohelp should succeed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[then("stdout contains agent-context JSON for the fixture")]
fn output_contains_agent_context(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let json = read_agent_context(orthohelp_context)?;
    assert_agent_context_contract(&json, &["fixture"], "Orthohelp fixture configuration.")
}

#[then("stdout contains agent-context JSON for the simple fixture")]
fn output_contains_simple_agent_context(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let json = read_agent_context(orthohelp_context)?;
    assert_agent_context_contract(&json, &["simple_fixture"], "Simple fixture configuration.")
}

#[then("stdout contains nested agent-context command paths for the fixture")]
fn output_contains_nested_agent_context_paths(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let json = read_agent_context(orthohelp_context)?;
    assert_agent_context_contract(&json, &["nested_fixture"], "Nested fixture command tree.")?;
    expect_command_path(&json, &["nested_fixture", "greet"])?;
    expect_command_path(&json, &["nested_fixture", "admin", "audit"])?;
    Ok(())
}

fn assert_agent_context_contract(
    json: &Value,
    expected_path: &[&str],
    expected_summary: &str,
) -> StepResult<()> {
    expect_str_field(json, JsonField::SchemaVersion, EXPECTED_SCHEMA_VERSION)?;
    let kind = string_field(json, JsonField::Kind)?;
    let expected_suffix = format!(".{AGENT_CONTEXT_KIND_SUFFIX}");
    if !kind.ends_with(&expected_suffix) {
        return Err(format!("kind should end with {expected_suffix}, got {kind}").into());
    }
    let command = json
        .get(JsonField::Commands.as_str())
        .and_then(Value::as_array)
        .and_then(|commands| commands.first())
        .ok_or("first command missing")?;
    expect_string_array_field(command, JsonField::Path, expected_path)?;
    expect_str_field(command, JsonField::Summary, expected_summary)?;
    expect_non_empty_array(command, JsonField::Inputs)?;
    expect_first_input_required_boolean(command)
}

fn expect_first_input_required_boolean(command: &Value) -> StepResult<()> {
    let input = command
        .get(JsonField::Inputs.as_str())
        .and_then(Value::as_array)
        .and_then(|inputs| inputs.first())
        .ok_or("first command input missing")?;
    if input
        .get(JsonField::Required.as_str())
        .is_some_and(Value::is_boolean)
    {
        Ok(())
    } else {
        Err("first command input required should be a boolean".into())
    }
}

fn read_agent_context(orthohelp_context: &OrthoHelpContext) -> StepResult<Value> {
    let stdout = orthohelp_context
        .last_output
        .with_ref(|output| output.stdout.clone())
        .ok_or("last_output should be set")?;
    assert_one_compact_json_line(&stdout)?;
    Ok(serde_json::from_slice(&stdout)?)
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

fn string_field(value: &Value, field: JsonField) -> StepResult<&str> {
    value
        .get(field.as_str())
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} field missing").into())
}

fn expect_str_field(value: &Value, field: JsonField, expected: &str) -> StepResult<()> {
    let actual = string_field(value, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} should be {expected}, got {actual}").into())
    }
}

fn expect_string_array_field(value: &Value, field: JsonField, expected: &[&str]) -> StepResult<()> {
    let actual = string_array_field(value, field)?;
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        Ok(())
    } else {
        Err(format!("{field} should be {expected:?}, got {actual:?}").into())
    }
}

fn expect_command_path(value: &Value, expected: &[&str]) -> StepResult<()> {
    let paths = command_paths(value)?;
    if paths
        .iter()
        .any(|path| path.iter().map(String::as_str).eq(expected.iter().copied()))
    {
        Ok(())
    } else {
        Err(format!("expected command path {expected:?}, got {paths:?}").into())
    }
}

fn command_paths(value: &Value) -> StepResult<Vec<Vec<String>>> {
    value
        .get(JsonField::Commands.as_str())
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{} field missing", JsonField::Commands))?
        .iter()
        .map(|command| string_array_field(command, JsonField::Path))
        .collect()
}

fn expect_non_empty_array(value: &Value, field: JsonField) -> StepResult<()> {
    match value.get(field.as_str()).and_then(Value::as_array) {
        Some(items) if !items.is_empty() => Ok(()),
        Some(_) => Err(format!("{field} should not be empty").into()),
        None => Err(format!("{field} field missing").into()),
    }
}

fn string_array_field(value: &Value, field: JsonField) -> StepResult<Vec<String>> {
    value
        .get(field.as_str())
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

/// Finds the command whose `path` array equals `path_segments`.
///
/// Comparison is element-wise over the whole path, so a subcommand is not
/// matched by a shorter prefix or a shared leaf name.
fn command_by_path<'a>(value: &'a Value, path_segments: &[&str]) -> Option<&'a Value> {
    value
        .get(JsonField::Commands.as_str())
        .and_then(Value::as_array)?
        .iter()
        .find(|command| {
            command
                .get(JsonField::Path.as_str())
                .and_then(Value::as_array)
                .is_some_and(|path| {
                    path.iter()
                        .filter_map(Value::as_str)
                        .eq(path_segments.iter().copied())
                })
        })
}

/// Asserts one string field of a command identified by its full path.
///
/// Fails with the offending path and field name when either the command or the
/// field is missing, so a mismatch is traceable to the fixture rather than the
/// assertion.
fn assert_command_string_field(
    orthohelp_context: &mut OrthoHelpContext,
    path: &[&str],
    field: JsonField,
    expected: &str,
) -> StepResult<()> {
    let json = read_agent_context(orthohelp_context)?;
    let command =
        command_by_path(&json, path).ok_or_else(|| format!("command path {path:?} missing"))?;
    let actual = string_field(command, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("command {path:?} {field} should be {expected}, got {actual}").into())
    }
}

/// The fixture's declared interactive mode survives into the artefact.
#[then("the command admin purge reports interaction mode interactive")]
fn admin_purge_interaction_interactive(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "admin", "purge"],
        JsonField::InteractionMode,
        "interactive",
    )
}

/// A declared destructive mutation is reported verbatim, not normalised.
#[then("the command admin purge reports mutation effect delete")]
fn admin_purge_mutation_delete(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "admin", "purge"],
        JsonField::MutationEffect,
        "delete",
    )
}

/// The declared bypass flag reaches agents so they can actually pass it.
#[then("the command admin purge reports bypass flag --force")]
fn admin_purge_bypass_force(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "admin", "purge"],
        JsonField::BypassFlag,
        "--force",
    )
}

/// A destructive command with no bypass still reports its mutation.
///
/// This is the fixture command the policy check flags in deny mode, so the
/// artefact must describe it accurately for the report to make sense.
#[then("the command admin prune reports mutation effect delete")]
fn admin_prune_mutation_delete(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "admin", "prune"],
        JsonField::MutationEffect,
        "delete",
    )
}

/// A declared non-interactive command reports that mode, not a guess.
#[then("the command greet reports interaction mode non_interactive")]
fn greet_interaction_non_interactive(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "greet"],
        JsonField::InteractionMode,
        "non_interactive",
    )
}

/// The read-only boundary is reported for a command that changes nothing.
#[then("the command greet reports mutation effect read_only")]
fn greet_mutation_read_only(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "greet"],
        JsonField::MutationEffect,
        "read_only",
    )
}

/// A command declaring nothing reports `unknown` rather than a default.
///
/// This is the no-inference rule observed end-to-end: the bridge must not
/// guess `non_interactive` merely because `version` sounds harmless.
#[then("the command version reports interaction mode unknown")]
fn version_interaction_unknown(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "version"],
        JsonField::InteractionMode,
        "unknown",
    )
}

/// The `submit` boundary survives the whole attribute → IR → bridge path.
///
/// `submit` is the least common accepted mutation, so it is asserted through
/// the real fixture rather than only in unit-level tables. A derive or bridge
/// arm that collapsed it to `unknown` would still pass every other case.
#[then("the command admin grant-access reports mutation effect submit")]
fn admin_grant_access_mutation_submit(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_command_string_field(
        orthohelp_context,
        &["nested_fixture", "admin", "grant-access"],
        JsonField::MutationEffect,
        "submit",
    )
}
