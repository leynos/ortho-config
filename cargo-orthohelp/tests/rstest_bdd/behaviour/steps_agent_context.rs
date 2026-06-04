//! Agent-context JSON step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements the `when`/`then` steps that exercise `--format agent-context`
//! against the fixture crate and assert the compact JSON contract that agents
//! consume.

use std::io::Read;

use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp};

#[when("I run cargo-orthohelp with format agent-context for the fixture")]
fn run_with_format_agent_context(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--format",
            "agent-context",
            "--package",
            "orthohelp_fixture",
        ],
    )?;
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
    expect_str_field(&json, "schema_version", "1")?;
    expect_str_field(&json, "kind", "orthohelp_fixture.agent_context")?;
    let command = json
        .get("commands")
        .and_then(Value::as_array)
        .and_then(|commands| commands.first())
        .ok_or("first command missing")?;
    expect_string_array_field(command, "path", &["fixture"])?;
    expect_str_field(command, "summary", "Orthohelp fixture configuration.")?;
    expect_non_empty_array(command, "inputs")?;
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

fn string_field<'a>(value: &'a Value, field: &str) -> StepResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} field missing").into())
}

fn expect_str_field(value: &Value, field: &str, expected: &str) -> StepResult<()> {
    let actual = string_field(value, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} should be {expected}, got {actual}").into())
    }
}

fn expect_string_array_field(value: &Value, field: &str, expected: &[&str]) -> StepResult<()> {
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

fn expect_non_empty_array(value: &Value, field: &str) -> StepResult<()> {
    match value.get(field).and_then(Value::as_array) {
        Some(items) if !items.is_empty() => Ok(()),
        Some(_) => Err(format!("{field} should not be empty").into()),
        None => Err(format!("{field} field missing").into()),
    }
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
