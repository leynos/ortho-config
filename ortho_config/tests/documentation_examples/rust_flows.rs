//! Behavioural checks for the public Rust examples and console commands.

use super::{EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile, documented_example};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub(super) fn assert_error_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    let output = workspace.run(ExampleId("guide-errors"), std::iter::empty::<&str>())?;
    ensure!(
        output.status.success(),
        "guide-errors should handle its error"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains("invalid value"),
        "guide-errors should render clap's parse failure"
    );
    Ok(())
}

pub(super) fn assert_tracing_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    let output = workspace.run(ExampleId("guide-tracing"), std::iter::empty::<&str>())?;
    ensure!(output.status.success(), "guide-tracing should succeed");
    ensure!(
        output.stdout == b"port=8080\n",
        "guide-tracing stdout should stay clean"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains("discovery.attempt"),
        "guide-tracing should emit debug discovery diagnostics"
    );
    Ok(())
}

pub(super) fn assert_console_flows(workspace: &mut ExampleWorkspace) -> Result<()> {
    let readme_console = documented_example("readme-run")?;
    ensure!(
        readme_console.body
            == "$ cargo run -- --host 0.0.0.0 --port 3000\nListening on 0.0.0.0:3000\n",
        "README command contract drifted"
    );

    let config_file = documented_example("guide-file")?;
    workspace.write_run_file(
        ExampleId("guide-first-cli"),
        RunFile {
            path: Path::new(".acme.toml"),
            contents: &config_file.body,
        },
    )?;
    let output = workspace.run_with_environment(
        ExampleId("guide-first-cli"),
        ["--port", "3000"],
        [EnvironmentVariable {
            name: "ACME_HOST",
            value: "api.internal",
        }],
    )?;
    ensure!(
        output.status.success(),
        "file-backed guide command should succeed"
    );
    let stdout = String::from_utf8(output.stdout).context("guide output should be UTF-8")?;
    ensure!(
        stdout == "host=api.internal port=3000 log_level=debug\n",
        "file-backed guide output differed: {stdout:?}"
    );
    let guide_console = documented_example("guide-file-run")?;
    ensure!(
        guide_console.body
            == concat!(
                "$ ACME_HOST=api.internal cargo run -- --port 3000\n",
                "host=api.internal port=3000 log_level=debug\n",
            ),
        "user's-guide command contract drifted"
    );
    Ok(())
}
