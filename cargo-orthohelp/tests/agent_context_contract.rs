//! Integration contracts for emitted agent-context JSON.
//!
//! These tests deliberately assert the stable machine contract separately from
//! the golden snapshots. They exercise the compiled binary and validate the
//! schema identity and command shape that downstream agents consume.

mod fixtures;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use fixtures::profile_redaction_sentinels::{REDACTION_ROOT_TYPE, all_protected_sentinels};
use ortho_config::AGENT_CONTEXT_KIND_SUFFIX;
use rstest::rstest;
use serde_json::Value;
use std::error::Error;
use std::process::{Command, Output};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const EXPECTED_SCHEMA_VERSION: &str = "1";

#[rstest]
#[case::simple(
    Some("orthohelp_fixture::SimpleFixtureConfig"),
    &["simple_fixture"],
    "Simple fixture configuration."
)]
#[case::enum_root(
    None,
    &["fixture"],
    "Orthohelp fixture configuration."
)]
#[case::nested(
    Some("orthohelp_fixture::NestedFixtureConfig"),
    &["nested_fixture"],
    "Nested fixture command tree."
)]
fn emitted_agent_context_has_stable_contract(
    #[case] root_type: Option<&str>,
    #[case] expected_path: &[&str],
    #[case] expected_summary: &str,
) -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_agent_context(&out_dir, root_type)?;
    ensure_success(&output)?;
    assert_debug_trace_is_stderr(&output)?;

    let context = read_stdout_agent_context(&output)?;
    assert_string_field(&context, "schema_version", EXPECTED_SCHEMA_VERSION)?;
    assert_kind_suffix(&context)?;
    assert_first_command(&context, expected_path, expected_summary)?;
    if context != read_agent_context(&out_dir)? {
        return Err("agent-context stdout should match its generated artefact".into());
    }
    Ok(())
}

/// Protected values must stay out of every readable surface, including traces.
///
/// The binary runs with `RUST_LOG=debug`, so a tracing event that formatted a
/// default or an example would be captured here alongside the artefact.
#[test]
fn redaction_keeps_protected_values_out_of_streams_and_artefact() -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_agent_context(&out_dir, Some(REDACTION_ROOT_TYPE))?;
    ensure_success(&output)?;
    assert_debug_trace_is_stderr(&output)?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let artefact = read_agent_context_text(&out_dir)?;
    assert_no_protected_sentinel(&[
        ("stdout", &stdout),
        ("stderr", &stderr),
        ("artefact", &artefact),
    ])?;

    let context = read_stdout_agent_context(&output)?;
    let redaction = context
        .get("profiles")
        .and_then(|profiles| profiles.get("redaction"))
        .and_then(Value::as_array)
        .ok_or("profiles.redaction should be an array")?;
    // Comparing identities rather than a count keeps this contract from
    // passing when the fixture drops a record and adds an unrelated one.
    // Records are ordered by `(command_path, field_name)`, so the root's
    // fields precede the nested command's.
    let expected_fields = [
        "api_key",
        "credential_file",
        "inferred_secret",
        "locator",
        "token",
        "replacement",
    ];
    let actual: Vec<&str> = redaction
        .iter()
        .filter_map(|record| record.get("field_name").and_then(Value::as_str))
        .collect();
    if actual != expected_fields {
        return Err(
            format!("expected protected fields {expected_fields:?}, got {actual:?}").into(),
        );
    }
    // Non-vacuity: the same document keeps the public description and default.
    for expected in ["Redaction fixture command tree.", "eu-west"] {
        if !stdout.contains(expected) {
            return Err(format!("stdout should retain {expected:?}").into());
        }
    }
    Ok(())
}

fn assert_no_protected_sentinel(streams: &[(&str, &str)]) -> TestResult {
    for sentinel in all_protected_sentinels() {
        for (label, content) in streams {
            if content.contains(sentinel) {
                return Err(format!("protected sentinel {sentinel} leaked into {label}").into());
            }
        }
    }
    Ok(())
}

fn read_agent_context_text(out_dir: &TempDir) -> TestResult<String> {
    let out_path = Utf8PathBuf::from_path_buf(out_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    let directory = Dir::open_ambient_dir(out_path, ambient_authority())?;
    Ok(directory.read_to_string("agent-context.json")?)
}

fn run_agent_context(out_dir: &TempDir, root_type: Option<&str>) -> TestResult<Output> {
    let executable = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(executable.as_str());
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        // Debug tracing must not contaminate the machine-readable stdout.
        .env("RUST_LOG", "debug")
        .args([
            "orthohelp",
            "--out-dir",
            out_dir
                .path()
                .to_str()
                .ok_or("output path should be UTF-8")?,
            "--format",
            "agent-context",
            "--package",
            "orthohelp_fixture",
        ]);
    if let Some(selected_root_type) = root_type {
        command.args(["--root-type", selected_root_type]);
    }
    Ok(command.output()?)
}

fn ensure_success(output: &Output) -> TestResult {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo-orthohelp should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

fn assert_debug_trace_is_stderr(output: &Output) -> TestResult {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("cargo-orthohelp dispatched via Cargo external-subcommand") {
        Ok(())
    } else {
        Err("RUST_LOG=debug should route tracing events to stderr".into())
    }
}

fn read_agent_context(out_dir: &TempDir) -> TestResult<Value> {
    let out_path = Utf8PathBuf::from_path_buf(out_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    let directory = Dir::open_ambient_dir(out_path, ambient_authority())?;
    let json = directory.read_to_string("agent-context.json")?;
    Ok(serde_json::from_str(&json)?)
}

fn read_stdout_agent_context(output: &Output) -> TestResult<Value> {
    assert_one_compact_json_line(&output.stdout)?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn assert_one_compact_json_line(stdout: &[u8]) -> TestResult {
    if stdout
        .strip_suffix(b"\n")
        .is_some_and(|document| !document.contains(&b'\n'))
    {
        Ok(())
    } else {
        Err("stdout should contain one compact JSON document and a trailing newline".into())
    }
}

fn assert_kind_suffix(context: &Value) -> TestResult {
    let kind = string_field(context, "kind")?;
    let expected_suffix = format!(".{AGENT_CONTEXT_KIND_SUFFIX}");
    if kind.ends_with(&expected_suffix) {
        Ok(())
    } else {
        Err(format!("kind should end with {expected_suffix:?}, got {kind:?}").into())
    }
}

fn assert_first_command(
    context: &Value,
    expected_path: &[&str],
    expected_summary: &str,
) -> TestResult {
    let commands = context
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("commands should be an array")?;
    let command = commands.first().ok_or("commands should not be empty")?;
    assert_command_path(command, expected_path)?;
    assert_string_field(command, "summary", expected_summary)?;
    assert_first_input(command)
}

fn assert_command_path(command: &Value, expected_path: &[&str]) -> TestResult {
    let path = command
        .get("path")
        .and_then(Value::as_array)
        .ok_or("first command path should be an array")?;
    let actual_path = path
        .iter()
        .map(|segment| {
            segment
                .as_str()
                .ok_or("command path entries should be strings")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if actual_path != expected_path {
        return Err(format!("expected command path {expected_path:?}, got {actual_path:?}").into());
    }
    Ok(())
}

fn assert_first_input(command: &Value) -> TestResult {
    let inputs = command
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or("first command inputs should be an array")?;
    let input = inputs
        .first()
        .ok_or("first command inputs should not be empty")?;
    for field in ["name", "long", "value_type", "required", "enum_values"] {
        if input.get(field).is_none() {
            return Err(format!("first command input should contain {field:?}").into());
        }
    }
    if !input.get("required").is_some_and(Value::is_boolean) {
        return Err("first command input required should be a boolean".into());
    }
    Ok(())
}

fn assert_string_field(value: &Value, field: &str, expected: &str) -> TestResult {
    let actual = string_field(value, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} should be {expected:?}, got {actual:?}").into())
    }
}

fn string_field<'a>(value: &'a Value, field: &str) -> TestResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} should be a string").into())
}
