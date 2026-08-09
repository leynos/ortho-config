//! Compile-and-run contracts for Rust and console examples in public docs.

mod documentation_examples;
#[path = "documentation_examples/workspace.rs"]
mod workspace;

use anyhow::{Context, Result, ensure};
use documentation_examples::documented_example;
use std::path::Path;
use workspace::{DependencyAlias, EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile};

const STANDARD_RUST_EXAMPLES: &[&str] = &[
    "readme-main",
    "guide-first-cli",
    "guide-discovery",
    "guide-hermetic-discovery",
    "guide-load-first-outcomes",
    "guide-subcommand",
    "guide-errors",
    "guide-localization",
    "guide-tracing",
    "guide-orthohelp-metadata",
];

#[test]
fn documented_rust_compiles_and_runs() -> Result<()> {
    let workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    for id in STANDARD_RUST_EXAMPLES {
        workspace.add_binary(&documented_example(id)?)?;
    }
    workspace.build()?;

    assert_run(
        &workspace,
        ExampleId("readme-main"),
        ["--host", "0.0.0.0", "--port", "3000"],
        "Listening on 0.0.0.0:3000\n",
    )?;
    assert_run(
        &workspace,
        ExampleId("guide-first-cli"),
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
    assert_run(&workspace, ExampleId("guide-discovery"), [], "port=8080\n")?;
    assert_run(&workspace, ExampleId("guide-hermetic-discovery"), [], "")?;
    assert_run(
        &workspace,
        ExampleId("guide-subcommand"),
        ["serve", "--port", "3000"],
        "port=Some(3000)\n",
    )?;
    assert_run(&workspace, ExampleId("guide-localization"), [], "")?;
    assert_tracing_flow(&workspace)?;
    assert_run(&workspace, ExampleId("guide-orthohelp-metadata"), [], "")?;

    let error_output = workspace.run(ExampleId("guide-errors"), std::iter::empty::<&str>())?;
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

fn assert_tracing_flow(workspace: &ExampleWorkspace) -> Result<()> {
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

#[test]
fn aliased_dependency_example_compiles_and_runs() -> Result<()> {
    let workspace = ExampleWorkspace::new(DependencyAlias("config_layer"))?;
    workspace.add_binary(&documented_example("guide-alias-derive")?)?;
    workspace.build()?;
    assert_run(&workspace, ExampleId("guide-alias-derive"), [], "")
}

#[test]
fn workspace_rejects_paths_outside_an_example_directory() -> Result<()> {
    let workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;

    let id_error = workspace
        .run(ExampleId("../escape"), std::iter::empty::<&str>())
        .expect_err("path-like example identifiers should be rejected");
    ensure!(
        id_error
            .to_string()
            .contains("not a safe documented example identifier"),
        "unexpected identifier error: {id_error}"
    );

    for path in [Path::new(""), Path::new("../escape")] {
        let path_error = workspace
            .write_run_file(ExampleId("readme-main"), RunFile { path, contents: "" })
            .expect_err("unsafe run-file paths should be rejected");
        ensure!(
            path_error
                .to_string()
                .contains("must stay within the example directory"),
            "unexpected run-file error: {path_error}"
        );
    }

    Ok(())
}

fn assert_console_flows(workspace: &ExampleWorkspace) -> Result<()> {
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

fn assert_run<const N: usize>(
    workspace: &ExampleWorkspace,
    ExampleId(id): ExampleId<'_>,
    args: [&str; N],
    expected_stdout: &str,
) -> Result<()> {
    let output = workspace.run(ExampleId(id), args)?;
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
