//! BDD coverage for the `hello_world context` command.

use anyhow::{Result, ensure};
use assert_cmd::Command as AssertCommand;
use ortho_config::serde_json::{self, Value};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, scenarios, then, when};
use test_helpers::text::normalize_scalar;

#[derive(Debug, Default, ScenarioState)]
struct AgentContextState {
    stdout: Slot<String>,
    stderr: Slot<String>,
    success: Slot<bool>,
}

#[fixture]
fn agent_context_state() -> AgentContextState {
    AgentContextState::default()
}

scenarios!(
    "tests/features/agent_context.feature",
    fixtures = [agent_context_state: AgentContextState]
);

#[when("I run the hello world example with arguments {arguments}")]
fn run_with_args(agent_context_state: &AgentContextState, arguments: String) -> Result<()> {
    let normalized_arguments = normalize_scalar(&arguments);
    #[expect(
        deprecated,
        reason = "cargo_bin is the standard assert_cmd API in this test suite"
    )]
    let mut command = AssertCommand::cargo_bin("hello_world")?;
    let args = shlex::split(&normalized_arguments).ok_or_else(|| {
        anyhow::anyhow!("failed to split scenario arguments: {normalized_arguments:?}")
    })?;
    command.args(args);
    let output = command.output()?;

    agent_context_state
        .stdout
        .set(String::from_utf8_lossy(&output.stdout).into_owned());
    agent_context_state
        .stderr
        .set(String::from_utf8_lossy(&output.stderr).into_owned());
    agent_context_state.success.set(output.status.success());
    Ok(())
}

#[then("the command succeeds")]
fn command_succeeds(agent_context_state: &AgentContextState) -> Result<()> {
    ensure!(
        agent_context_state.success.with_ref(|success| *success) == Some(true),
        "expected command to succeed; stderr was {:?}",
        agent_context_state.stderr.with_ref(Clone::clone)
    );
    Ok(())
}

#[then("stdout contains {expected_stdout}")]
fn stdout_contains(agent_context_state: &AgentContextState, expected_stdout: String) -> Result<()> {
    let normalized_expected_stdout = normalize_scalar(&expected_stdout);
    let stdout = agent_context_state
        .stdout
        .with_ref(Clone::clone)
        .unwrap_or_default();
    ensure!(
        stdout.contains(&normalized_expected_stdout),
        "expected stdout to contain {normalized_expected_stdout:?}; stdout was {stdout:?}"
    );
    Ok(())
}

#[then("stdout contains a valid agent-context payload with kind {expected_kind}")]
fn stdout_contains_agent_context_kind(
    agent_context_state: &AgentContextState,
    expected_kind: String,
) -> Result<()> {
    let normalized_expected_kind = normalize_scalar(&expected_kind);
    let stdout = agent_context_state
        .stdout
        .with_ref(Clone::clone)
        .unwrap_or_default();
    let payload: Value = serde_json::from_str(&stdout)?;

    ensure!(
        payload.get("schema_version").and_then(Value::as_str) == Some("1"),
        "agent-context schema_version should be 1; payload was {payload:?}"
    );
    ensure!(
        payload.get("kind").and_then(Value::as_str) == Some(normalized_expected_kind.as_str()),
        "agent-context kind should be {normalized_expected_kind:?}; payload was {payload:?}"
    );
    Ok(())
}
