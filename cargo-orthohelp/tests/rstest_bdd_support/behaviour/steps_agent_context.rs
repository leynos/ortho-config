//! Agent-context JSON step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements the `when`/`then` steps that exercise `--format agent-context`
//! against the fixture crate and assert the compact JSON contract that agents
//! consume.

use std::fmt;
use std::io::Read;

use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp};

#[derive(Debug, Clone, Copy)]
pub(super) enum JsonField {
    SchemaVersion,
    Kind,
    Commands,
    Path,
    Summary,
    Inputs,
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

#[then("the output contains agent-context JSON for the fixture")]
fn output_contains_agent_context(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output_succeeded = orthohelp_context
        .last_output
        .with_ref(|output| output.status.success())
        .ok_or("last_output should be set")?;
    if !output_succeeded {
        return Err("cargo-orthohelp should succeed".into());
    }

    let json = read_agent_context(orthohelp_context)?;
    expect_str_field(&json, JsonField::SchemaVersion, "1")?;
    expect_str_field(&json, JsonField::Kind, "orthohelp_fixture.agent_context")?;
    let command = json
        .get(JsonField::Commands.as_str())
        .and_then(Value::as_array)
        .and_then(|commands| commands.first())
        .ok_or("first command missing")?;
    expect_string_array_field(command, JsonField::Path, &["fixture"])?;
    expect_str_field(
        command,
        JsonField::Summary,
        "Orthohelp fixture configuration.",
    )?;
    expect_non_empty_array(command, JsonField::Inputs)?;
    Ok(())
}

#[then("the output contains nested agent-context command paths for the fixture")]
fn output_contains_nested_agent_context_paths(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let output_succeeded = orthohelp_context
        .last_output
        .with_ref(|output| output.status.success())
        .ok_or("last_output should be set")?;
    if !output_succeeded {
        return Err("cargo-orthohelp should succeed".into());
    }

    let json = read_agent_context(orthohelp_context)?;
    expect_str_field(&json, JsonField::SchemaVersion, "1")?;
    expect_str_field(&json, JsonField::Kind, "orthohelp_fixture.agent_context")?;
    expect_command_path(&json, &["nested_fixture", "greet"])?;
    expect_command_path(&json, &["nested_fixture", "admin", "audit"])?;
    Ok(())
}

fn read_agent_context(orthohelp_context: &mut OrthoHelpContext) -> StepResult<Value> {
    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open("agent-context.json")?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    Ok(serde_json::from_str(&buffer)?)
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
