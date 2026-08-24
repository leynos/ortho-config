//! Compile-and-run contracts for Rust and console examples in public docs.

mod documentation_examples;
#[path = "documentation_examples/process_runner.rs"]
mod process_runner;
#[path = "documentation_examples/workspace.rs"]
mod workspace;

use anyhow::{Context, Result, ensure};
use documentation_examples::{DocumentedExample, documented_example};
use std::path::{Path, PathBuf};
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
    "api-guide-policy-report-json",
    "api-guide-agent-context-json",
];

#[test]
fn documented_rust_compiles_and_runs() -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    for id in STANDARD_RUST_EXAMPLES {
        workspace.add_binary(documented_example(id)?)?;
    }
    workspace.add_binary(&environment_probe())?;
    workspace.build()?;

    assert_standard_example_runs(&mut workspace)?;
    assert_tracing_flow(&mut workspace)?;
    assert_run(
        &mut workspace,
        ExampleId("guide-orthohelp-metadata"),
        [],
        "field=host\n",
    )?;
    assert_serialized_json(
        &mut workspace,
        ExampleId("api-guide-policy-report-json"),
        "version",
    )?;
    assert_serialized_json(
        &mut workspace,
        ExampleId("api-guide-agent-context-json"),
        "schema_version",
    )?;
    assert_sanitized_binary_environment(&mut workspace)?;

    assert_error_flow(&mut workspace)?;

    assert_console_flows(&mut workspace)
}

fn assert_standard_example_runs(workspace: &mut ExampleWorkspace) -> Result<()> {
    assert_run(
        workspace,
        ExampleId("readme-main"),
        ["--host", "0.0.0.0", "--port", "3000"],
        "Listening on 0.0.0.0:3000\n",
    )?;
    assert_run(
        workspace,
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
    assert_run(workspace, ExampleId("guide-discovery"), [], "port=8080\n")?;
    assert_run(
        workspace,
        ExampleId("guide-hermetic-discovery"),
        [],
        "candidate=/srv/acme/server.toml\n",
    )?;
    assert_run(
        workspace,
        ExampleId("guide-load-first-outcomes"),
        [],
        "discovery=absent\n",
    )?;
    assert_run(
        workspace,
        ExampleId("guide-subcommand"),
        ["serve", "--port", "3000"],
        "port=Some(3000)\n",
    )?;
    assert_run(
        workspace,
        ExampleId("guide-localization"),
        [],
        "verbose=true\n",
    )
}

fn assert_error_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
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

fn assert_tracing_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
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
    let mut workspace = ExampleWorkspace::new(DependencyAlias("config_layer"))?;
    workspace.add_binary(documented_example("guide-alias-derive")?)?;
    workspace.build()?;
    assert_run(
        &mut workspace,
        ExampleId("guide-alias-derive"),
        [],
        "port=8080\n",
    )
}

#[test]
fn workspace_rejects_paths_outside_an_example_directory() -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;

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

fn environment_probe() -> DocumentedExample {
    DocumentedExample {
        id: "environment-probe".to_owned(),
        language: "rust".to_owned(),
        body: concat!(
            "fn main() {\n",
            "    let current = std::env::current_dir().expect(\"read current directory\");\n",
            "    let home = std::env::var_os(\"HOME\").expect(\"HOME should be set\");\n",
            "    let xdg = std::env::var_os(\"XDG_CONFIG_HOME\").expect(\"XDG should be set\");\n",
            "    println!(\"current={}\", current.display());\n",
            "    println!(\"home={}\", std::path::Path::new(&home).display());\n",
            "    println!(\"xdg={}\", std::path::Path::new(&xdg).display());\n",
            "    println!(\"path_present={}\", std::env::var_os(\"PATH\").is_some());\n",
            "}\n",
        )
        .to_owned(),
        source: "environment probe",
        line: 1,
    }
}

fn assert_sanitized_binary_environment(workspace: &mut ExampleWorkspace) -> Result<()> {
    ensure!(
        std::env::var_os("PATH").is_some(),
        "the parent test process should provide PATH"
    );
    let output = workspace.run(ExampleId("environment-probe"), std::iter::empty::<&str>())?;
    ensure!(output.status.success(), "environment probe should succeed");
    let stdout = String::from_utf8(output.stdout).context("environment probe output is UTF-8")?;
    let values = stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<std::collections::HashMap<_, _>>();
    let value = |name| {
        values
            .get(name)
            .copied()
            .with_context(|| format!("environment probe should report {name}"))
    };
    let current = PathBuf::from(value("current")?);
    ensure!(Path::new(value("home")?) == current);
    ensure!(Path::new(value("xdg")?) == current.join("xdg"));
    ensure!(value("path_present")? == "false");
    Ok(())
}

fn assert_console_flows(workspace: &mut ExampleWorkspace) -> Result<()> {
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
    workspace: &mut ExampleWorkspace,
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

fn assert_serialized_json(
    workspace: &mut ExampleWorkspace,
    ExampleId(id): ExampleId<'_>,
    version_field: &str,
) -> Result<()> {
    let output = workspace.run(ExampleId(id), std::iter::empty::<&str>())?;
    ensure!(
        output.status.success(),
        "{id} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: ortho_config::serde_json::Value =
        ortho_config::serde_json::from_slice(&output.stdout)
            .with_context(|| format!("{id} should print JSON"))?;
    ensure!(
        payload.get(version_field).is_some(),
        "{id} JSON should contain {version_field}"
    );
    Ok(())
}
