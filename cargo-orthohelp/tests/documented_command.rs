//! Execute the published CLI command using Cargo's already-built binary.
//!
//! This test belongs to the CLI package so Cargo supplies the exact executable
//! path. Shared documentation support owns the private snapshot, environment,
//! subprocess tree, fixed scenario deadline, and bounded diagnostics.

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
use std::path::Path;

#[test]
fn documented_orthohelp_command_generates_agent_context() -> Result<()> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace repository")?;
    let state = repository.join("target/documentation-workspaces");
    Dir::create_ambient_dir_all(&state, ambient_authority())?;
    let output = tempfile::Builder::new()
        .prefix("documented-command-")
        .tempdir_in(state)?;
    orthohelp_workflow::assert_agent_context(
        repository,
        Path::new(env!("CARGO_BIN_EXE_cargo-orthohelp")),
        output.path(),
    )?;
    let directory = Dir::open_ambient_dir(output.path(), ambient_authority())?;
    let json = directory.read_to_string("agent-context.json")?;
    let payload: serde_json::Value = serde_json::from_str(&json)?;
    ensure!(payload.get("package").and_then(serde_json::Value::as_str) == Some("hello_world"));
    ensure!(
        payload.get("kind").and_then(serde_json::Value::as_str)
            == Some("hello_world.agent_context")
    );
    Ok(())
}
