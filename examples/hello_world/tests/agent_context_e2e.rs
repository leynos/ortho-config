//! End-to-end tests for the `hello_world context --json` surface.

use assert_cmd::Command;
use ortho_config::serde_json::{self, Value};

fn hello_world_command() -> Command {
    #[expect(
        deprecated,
        clippy::expect_used,
        reason = "cargo_bin is the standard assert_cmd API and test panics are acceptable"
    )]
    Command::cargo_bin("hello_world").expect("binary should exist")
}

#[test]
fn context_json_emits_parseable_payload() {
    let output = hello_world_command()
        .args(["context", "--json"])
        .output()
        .expect("context command should execute");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let payload: Value = serde_json::from_str(&stdout).expect("stdout should parse as JSON");

    assert_eq!(
        payload.get("kind").and_then(Value::as_str),
        Some("hello_world.agent_context")
    );
    assert_eq!(
        payload.get("schema_version").and_then(Value::as_str),
        Some("1")
    );
}

#[test]
fn context_json_writes_only_to_stdout() {
    let output = hello_world_command()
        .args(["context", "--json"])
        .output()
        .expect("context command should execute");

    // This depends on no global tracing subscriber or logger being installed
    // before `Commands::Context` returns early in `main::run`.
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.is_empty());
}

#[test]
fn bare_context_prints_the_exact_json_pointer() {
    hello_world_command()
        .arg("context")
        .assert()
        .success()
        .stdout("Run `hello-world context --json` for JSON agent context.\n");
}

#[test]
fn context_exit_code_is_zero() {
    hello_world_command()
        .args(["context", "--json"])
        .assert()
        .success();
}
