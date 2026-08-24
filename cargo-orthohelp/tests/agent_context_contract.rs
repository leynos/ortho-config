//! Integration contracts for emitted agent-context JSON.
//!
//! These tests deliberately assert the stable machine contract separately from
//! the golden snapshots. They exercise the compiled binary and validate the
//! schema identity and command shape that downstream agents consume.

mod fixtures;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use ortho_config::{AGENT_CONTEXT_KIND_SUFFIX, ORTHO_AGENT_CONTEXT_SCHEMA_VERSION};
use rstest::rstest;
use serde_json::Value;
use std::error::Error;
use std::process::{Command, Output};
use std::sync::{LazyLock, Mutex, PoisonError};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

// The bridge's content-addressed workspace is shared by every fixture case.
// Serializing subprocesses prevents concurrent cases from rewriting its
// manifest while another case is compiling it.
static BRIDGE_BUILD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

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
    let bridge_build_guard = BRIDGE_BUILD_MUTEX
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let output = run_agent_context(&out_dir, root_type)?;
    drop(bridge_build_guard);
    ensure_success(&output)?;

    let context = read_agent_context(&out_dir)?;
    assert_string_field(
        &context,
        "schema_version",
        ORTHO_AGENT_CONTEXT_SCHEMA_VERSION,
    )?;
    assert_kind_suffix(&context)?;
    assert_first_command(&context, expected_path, expected_summary)?;
    Ok(())
}

fn run_agent_context(out_dir: &TempDir, root_type: Option<&str>) -> TestResult<Output> {
    let executable = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(executable.as_str());
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
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

fn read_agent_context(out_dir: &TempDir) -> TestResult<Value> {
    let out_path = Utf8PathBuf::from_path_buf(out_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    let directory = Dir::open_ambient_dir(out_path, ambient_authority())?;
    let json = directory.read_to_string("agent-context.json")?;
    Ok(serde_json::from_str(&json)?)
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
    assert_string_field(command, "summary", expected_summary)?;
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
