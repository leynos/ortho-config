//! Execute the published CLI commands using Cargo's already-built binary.
//!
//! This test belongs to the CLI package so Cargo supplies the exact executable
//! path. Shared documentation support owns the private snapshot, environment,
//! subprocess tree, fixed scenario deadline, and bounded diagnostics. The
//! published command text is pinned by the static documentation contracts in
//! `ortho_config/tests/documentation_examples/static_contracts.rs`; these tests
//! run the same flags and check what the documentation promises they produce.
//! The placeholder package `my-cli` in the policy examples is mapped onto the
//! workspace's policy fixture packages, whose manifests carry the documented
//! policy tables.

#[path = "../../ortho_config/tests/documentation_examples/build_state.rs"]
mod build_state;
#[path = "../../ortho_config/tests/documentation_examples/cargo_runner_support.rs"]
mod cargo_runner;
#[path = "../../ortho_config/tests/documentation_examples/orthohelp_workflow.rs"]
mod orthohelp_workflow;
#[path = "../../ortho_config/tests/documentation_examples/process_runner_state_support.rs"]
mod process_runner;

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use serde_json::Value;
use std::path::Path;
use std::process::Output;
use tempfile::TempDir;

/// A finished `cargo orthohelp` run and the directory it wrote into.
struct OrthohelpRun {
    output: Output,
    out_dir: TempDir,
}

impl OrthohelpRun {
    fn stdout(&self) -> Result<&str> {
        std::str::from_utf8(&self.output.stdout).context("cargo-orthohelp stdout is UTF-8")
    }

    fn read_artefact(&self, name: &str) -> Result<String> {
        Dir::open_ambient_dir(self.out_dir.path(), ambient_authority())?
            .read_to_string(name)
            .with_context(|| format!("read generated {name}"))
    }

    fn ensure_success(&self, flow: &str) -> Result<()> {
        ensure!(
            self.output.status.success(),
            "{flow} failed:\n{}",
            String::from_utf8_lossy(&self.output.stderr)
        );
        Ok(())
    }
}

/// Run `cargo orthohelp <args>` into a fresh output directory.
fn run_orthohelp(args: &[&str]) -> Result<OrthohelpRun> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace repository")?;
    let state = repository.join("target/documentation-workspaces");
    Dir::create_ambient_dir_all(&state, ambient_authority())?;
    let out_dir = tempfile::Builder::new()
        .prefix("documented-command-")
        .tempdir_in(state)?;
    let output = orthohelp_workflow::run_orthohelp(
        repository,
        Path::new(env!("CARGO_BIN_EXE_cargo-orthohelp")),
        args,
        out_dir.path(),
    )?;
    Ok(OrthohelpRun { output, out_dir })
}

/// `cargo orthohelp --package hello_world --format agent-context` writes the
/// agent context and, as the user's guide promises, streams the same document.
#[test]
fn documented_orthohelp_command_generates_agent_context() -> Result<()> {
    let run = run_orthohelp(&["--package", "hello_world", "--format", "agent-context"])?;
    run.ensure_success("documented agent-context command")?;
    let payload: Value = serde_json::from_str(&run.read_artefact("agent-context.json")?)?;
    ensure!(payload.get("package").and_then(Value::as_str) == Some("hello_world"));
    ensure!(payload.get("kind").and_then(Value::as_str) == Some("hello_world.agent_context"));

    let streamed: Value = serde_json::from_str(run.stdout()?.trim_end())?;
    ensure!(
        streamed == payload,
        "stdout should carry agent-context.json"
    );
    Ok(())
}

/// `cargo orthohelp --check-agent-native --package my-cli --out-dir out`
/// writes and streams one report, and the documented
/// `jq -e '.mode != "off"'` gate tells a configured package from one whose
/// policy table is missing or misspelt.
#[rstest]
#[case::configured("orthohelp_policy_warn_fixture", "warn", true)]
#[case::unconfigured("hello_world", "off", false)]
fn documented_policy_check_reports_the_effective_mode(
    #[case] package: &str,
    #[case] expected_mode: &str,
    #[case] is_gate_passing: bool,
) -> Result<()> {
    let run = run_orthohelp(&["--check-agent-native", "--package", package])?;
    run.ensure_success(&format!("policy check for {package}"))?;
    let report: Value = serde_json::from_str(&run.read_artefact("policy-report.json")?)?;
    let stdout = run.stdout()?;
    ensure!(
        stdout.ends_with('\n') && stdout.trim_end().lines().count() == 1,
        "stdout should hold one compact JSON document, got {stdout:?}"
    );
    ensure!(serde_json::from_str::<Value>(stdout)? == report);
    ensure!(
        String::from_utf8_lossy(&run.output.stderr)
            .trim_end()
            .lines()
            .count()
            == 1,
        "stderr should hold a one-line summary"
    );
    ensure!(
        !run.out_dir.path().join("agent-context.json").exists(),
        "a policy-only run should skip generation"
    );

    let mode = report.get("mode").and_then(Value::as_str);
    ensure!(
        mode == Some(expected_mode),
        "{package} reported mode {mode:?}"
    );
    ensure!((mode != Some("off")) == is_gate_passing);
    Ok(())
}
