//! Compile-and-run contracts for Rust and console examples in public docs.

#[path = "documentation_examples/api_guide.rs"]
mod api_guide;

#[path = "documentation_examples/boolean_override.rs"]
mod boolean_override;
mod documentation_examples;

#[path = "documentation_examples/env_alias_chain.rs"]
mod env_alias_chain;

#[path = "documentation_examples/process_runner_test_support.rs"]
mod process_runner;

#[path = "documentation_examples/rust_flows.rs"]
mod example_flows;

#[path = "documentation_examples/workspace.rs"]
mod workspace;

use anyhow::{Context, Result, ensure};
use boolean_override::assert_boolean_override_flow;
use documentation_examples::{DocumentedExample, documented_example};
use env_alias_chain::assert_env_alias_chain;
use std::path::{Path, PathBuf};
use workspace::{DependencyAlias, EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile};

const STANDARD_RUST_EXAMPLES: &[&str] = &[
    "readme-main",
    "guide-first-cli",
    "guide-discovery",
    "guide-hermetic-discovery",
    "guide-load-first-outcomes",
    "guide-compose-layers-with-sources",
    "guide-subcommand",
    "guide-subcommand-sources",
    "guide-selected-subcommand-sources",
    "guide-errors",
    "guide-localization",
    "guide-tracing",
    "guide-orthohelp-metadata",
    "guide-scoped-discovery",
    "api-guide-policy-report-constructors",
    "api-guide-policy-report-json",
    "api-guide-agent-context-constructor",
    "api-guide-agent-context-json",
    "guide-boolean-override",
];

/// Compiles and runs the public Rust examples, checking their documented contracts.
#[test]
#[serial_test::serial(documentation_build)]
fn documented_rust_compiles_and_runs() -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    for id in STANDARD_RUST_EXAMPLES {
        workspace.add_binary(documented_example(id)?)?;
    }
    workspace.add_binary(&environment_probe())?;
    api_guide::add_schema_version_probe(&mut workspace)?;
    workspace.build()?;

    let (policy_version, agent_context_version) = api_guide::schema_versions(&mut workspace)?;

    example_flows::assert_standard_example_runs(&mut workspace)?;
    assert_env_alias_chain(&mut workspace)?;
    example_flows::assert_tracing_flow(&mut workspace)?;
    assert_run(
        &mut workspace,
        ExampleId("guide-orthohelp-metadata"),
        [],
        "field=host\n",
    )?;
    assert_run(
        &mut workspace,
        ExampleId("api-guide-policy-report-constructors"),
        [],
        "",
    )?;
    assert_run(
        &mut workspace,
        ExampleId("api-guide-agent-context-constructor"),
        [],
        "",
    )?;
    api_guide::assert_json_schema_version(
        &mut workspace,
        ExampleId("api-guide-policy-report-json"),
        "version",
        &policy_version,
    )?;
    api_guide::assert_json_schema_version(
        &mut workspace,
        ExampleId("api-guide-agent-context-json"),
        "schema_version",
        &agent_context_version,
    )?;
    assert_sanitized_binary_environment(&mut workspace)?;

    assert_boolean_override_flow(&mut workspace)?;

    example_flows::assert_error_flow(&mut workspace)?;
    example_flows::assert_console_flows(&mut workspace)?;

    assert_run(
        &mut workspace,
        ExampleId("guide-selected-subcommand-sources"),
        [],
        "",
    )
}

#[test]
#[serial_test::serial(documentation_build)]
fn documented_cargo_external_subcommand_parses_both_invocation_forms() -> Result<()> {
    let example = documented_example("guide-cargo-external-subcommand")?;
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    workspace.add_binary_as("cargo-demo", example)?;
    workspace.build()?;

    let direct_success = workspace.run(ExampleId("cargo-demo"), ["demo", "--verbose"])?;
    ensure!(
        direct_success.status.success(),
        "direct documented Cargo helper invocation should succeed:\n{}",
        String::from_utf8_lossy(&direct_success.stderr)
    );

    let cargo_success = workspace.run_cargo_subcommand("demo", ["--verbose"])?;
    ensure!(
        cargo_success.status.success(),
        "Cargo-dispatched documented helper invocation should succeed:\n{}",
        String::from_utf8_lossy(&cargo_success.stderr)
    );

    let direct_help = workspace.run(ExampleId("cargo-demo"), ["--help"])?;
    ensure!(
        direct_help.status.success(),
        "direct helper help should succeed"
    );
    ensure!(
        String::from_utf8_lossy(&direct_help.stdout).contains("Usage: cargo <COMMAND>"),
        "direct helper help should render Cargo dispatch usage"
    );

    let cargo_help = workspace.run_cargo_subcommand("demo", ["--help"])?;
    ensure!(
        cargo_help.status.success(),
        "Cargo-dispatched helper help should succeed"
    );
    ensure!(
        String::from_utf8_lossy(&cargo_help.stdout).contains("Usage: cargo demo [OPTIONS]"),
        "Cargo-dispatched helper help should render subcommand usage"
    );

    let direct_failure = workspace.run(ExampleId("cargo-demo"), std::iter::empty::<&str>())?;
    ensure!(
        !direct_failure.status.success(),
        "direct helper should require a subcommand"
    );
    ensure!(
        String::from_utf8_lossy(&direct_failure.stderr).contains("Usage: cargo <COMMAND>"),
        "direct helper failure should render top-level usage"
    );

    let cargo_failure = workspace.run_cargo_subcommand("demo", ["--unknown"])?;
    ensure!(
        !cargo_failure.status.success(),
        "Cargo-dispatched helper should reject an unknown option"
    );
    ensure!(
        String::from_utf8_lossy(&cargo_failure.stderr).contains("Usage: cargo demo [OPTIONS]"),
        "Cargo-dispatched helper failure should render subcommand usage"
    );

    Ok(())
}

#[test]
#[serial_test::serial(documentation_build)]
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

/// One documented example invocation: its arguments and its environment.
///
/// The two vary independently at call sites, so grouping them under a name
/// keeps the assertion helpers short enough to read without spelling the
/// subjects out at every caller.
struct Invocation<'a, const N: usize, const E: usize> {
    args: [&'a str; N],
    environment: [EnvironmentVariable<'a>; E],
}

fn assert_run<const N: usize>(
    workspace: &mut ExampleWorkspace,
    ExampleId(id): ExampleId<'_>,
    args: [&str; N],
    expected_stdout: &str,
) -> Result<()> {
    assert_run_with_environment(
        workspace,
        ExampleId(id),
        Invocation {
            args,
            environment: [],
        },
        expected_stdout,
    )
}

/// Runs a documented example with an explicit invocation and asserts its stdout.
///
/// # Examples
///
/// ```ignore
/// assert_run_with_environment(
///     workspace,
///     ExampleId("guide-boolean-override"),
///     Invocation {
///         args: ["--excited=false"],
///         environment: [EnvironmentVariable { name: "ACME_EXCITED", value: "true" }],
///     },
///     "excited=false\n",
/// )?;
/// ```
fn assert_run_with_environment<const N: usize, const E: usize>(
    workspace: &mut ExampleWorkspace,
    ExampleId(id): ExampleId<'_>,
    Invocation { args, environment }: Invocation<'_, N, E>,
    expected_stdout: &str,
) -> Result<()> {
    let output = workspace.run_with_environment(ExampleId(id), args, environment)?;
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
