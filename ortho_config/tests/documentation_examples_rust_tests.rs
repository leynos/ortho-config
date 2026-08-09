//! Compile-and-run contracts for Rust and console examples in public docs.

mod documentation_examples;
#[path = "documentation_examples/workspace.rs"]
mod workspace;

use anyhow::{Context, Result, ensure};
use documentation_examples::documented_example;
use workspace::ExampleWorkspace;

const STANDARD_RUST_EXAMPLES: &[&str] = &[
    "readme-main",
    "guide-first-cli",
    "guide-discovery",
    "guide-hermetic-discovery",
    "guide-subcommand",
    "guide-errors",
    "guide-localization",
    "guide-tracing",
    "guide-orthohelp-metadata",
];

#[test]
fn documented_rust_compiles_and_runs() -> Result<()> {
    let workspace = ExampleWorkspace::new("ortho_config")?;
    for id in STANDARD_RUST_EXAMPLES {
        workspace.add_binary(&documented_example(id)?)?;
    }
    workspace.build()?;

    assert_run(
        &workspace,
        "readme-main",
        ["--host", "0.0.0.0", "--port", "3000"],
        "Listening on 0.0.0.0:3000\n",
    )?;
    assert_run(
        &workspace,
        "guide-first-cli",
        [
            "--host",
            "0.0.0.0",
            "--port",
            "3000",
            "--log-level",
            "debug",
        ],
        "host=0.0.0.0 port=3000 log_level=debug\n",
    )?;
    assert_run(&workspace, "guide-discovery", [], "port=8080\n")?;
    assert_run(&workspace, "guide-hermetic-discovery", [], "")?;
    assert_run(
        &workspace,
        "guide-subcommand",
        ["serve", "--port", "3000"],
        "port=Some(3000)\n",
    )?;
    assert_run(&workspace, "guide-localization", [], "")?;
    assert_run(&workspace, "guide-tracing", [], "port=8080\n")?;
    assert_run(&workspace, "guide-orthohelp-metadata", [], "")?;

    let error_output = workspace.run("guide-errors", std::iter::empty::<&str>())?;
    ensure!(
        error_output.status.success(),
        "guide-errors should handle its error"
    );
    ensure!(
        String::from_utf8_lossy(&error_output.stderr).contains("invalid value"),
        "guide-errors should render clap's parse failure"
    );

    assert_console_flows(&workspace)
}

#[test]
fn aliased_dependency_example_compiles_and_runs() -> Result<()> {
    let workspace = ExampleWorkspace::new("config_layer")?;
    workspace.add_binary(&documented_example("guide-alias-derive")?)?;
    workspace.build()?;
    assert_run(&workspace, "guide-alias-derive", [], "")
}

fn assert_console_flows(workspace: &ExampleWorkspace) -> Result<()> {
    let readme_console = documented_example("readme-run")?;
    ensure!(
        readme_console.body
            == "$ cargo run -- --host 0.0.0.0 --port 3000\nListening on 0.0.0.0:3000\n",
        "README command contract drifted"
    );

    let config_file = documented_example("guide-file")?;
    workspace.write_run_file("guide-first-cli", ".acme.toml", &config_file.body)?;
    let output = workspace.run("guide-first-cli", ["--port", "3000"])?;
    ensure!(
        output.status.success(),
        "file-backed guide command should succeed"
    );
    let stdout = String::from_utf8(output.stdout).context("guide output should be UTF-8")?;
    ensure!(
        stdout == "host=0.0.0.0 port=3000 log_level=debug\n",
        "file-backed guide output differed: {stdout:?}"
    );
    let guide_console = documented_example("guide-file-run")?;
    ensure!(
        guide_console.body
            == "$ cargo run -- --port 3000\nhost=0.0.0.0 port=3000 log_level=debug\n",
        "user's-guide command contract drifted"
    );
    Ok(())
}

fn assert_run<const N: usize>(
    workspace: &ExampleWorkspace,
    id: &str,
    args: [&str; N],
    expected_stdout: &str,
) -> Result<()> {
    let output = workspace.run(id, args)?;
    ensure!(
        output.status.success(),
        "{id} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout =
        String::from_utf8(output.stdout).with_context(|| format!("{id} stdout is UTF-8"))?;
    ensure!(
        stdout == expected_stdout,
        "{id} stdout differed: expected {expected_stdout:?}, got {stdout:?}"
    );
    Ok(())
}
