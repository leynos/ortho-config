//! Compile-and-run contracts for Rust and console examples in public docs.

#[path = "documentation_examples/api_guide.rs"]
mod api_guide;

#[path = "documentation_examples/boolean_override.rs"]
mod boolean_override;

#[path = "documentation_examples/cargo_flows.rs"]
mod cargo_flows;
mod documentation_examples;

#[path = "documentation_examples/env_alias_chain.rs"]
mod env_alias_chain;

#[path = "documentation_examples/environment_probe.rs"]
mod environment_probe;

#[path = "documentation_examples/guide_flows.rs"]
mod guide_flows;

#[path = "documentation_examples/migration_flows.rs"]
mod migration_flows;

#[path = "documentation_examples/process_runner_test_support.rs"]
mod process_runner;
#[path = "documentation_examples/process_runner_execution_policy.rs"]
mod process_runner_execution_policy;

#[path = "documentation_examples/rust_flows.rs"]
mod example_flows;

#[path = "documentation_examples/simple_runs.rs"]
mod simple_runs;

#[path = "documentation_examples/workspace.rs"]
mod workspace;

use anyhow::{Context, Result, ensure};
use boolean_override::assert_boolean_override_flow;
use documentation_examples::documented_example;
use env_alias_chain::assert_env_alias_chain;
use std::path::Path;
use workspace::{DependencyAlias, EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile};

const STANDARD_RUST_EXAMPLES: &[&str] = &[
    "readme-main",
    "guide-first-cli",
    "guide-collections",
    "guide-discovery",
    "guide-profiles",
    "guide-hermetic-discovery",
    "guide-load-first-outcomes",
    "guide-source-aware-load",
    "guide-compose-layers-with-sources",
    "guide-csv-env-with-source",
    "guide-subcommand",
    "guide-subcommand-merge-sources",
    "guide-subcommand-sources",
    "guide-selected-subcommand-sources",
    "guide-clap-only-parse",
    "guide-errors",
    "guide-localization",
    "guide-localization-ids",
    "guide-tracing",
    "guide-orthohelp-metadata",
    "guide-behaviour-metadata",
    "guide-check-command",
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
    let rust_examples = STANDARD_RUST_EXAMPLES
        .iter()
        .chain(migration_flows::MIGRATION_RUST_EXAMPLES);
    for id in rust_examples {
        workspace.add_binary(documented_example(id)?)?;
    }
    workspace.add_binary(&environment_probe::environment_probe())?;
    api_guide::add_schema_version_probe(&mut workspace)?;
    guide_flows::add_probes(&mut workspace)?;
    workspace.build()?;

    let (policy_version, agent_context_version) = api_guide::schema_versions(&mut workspace)?;

    simple_runs::assert_simple_runs(&mut workspace)?;
    assert_env_alias_chain(&mut workspace)?;
    example_flows::assert_tracing_flow(&mut workspace)?;
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
    environment_probe::assert_sanitized_binary_environment(&mut workspace)?;

    assert_boolean_override_flow(&mut workspace)?;
    guide_flows::assert_profile_flow(&mut workspace)?;
    guide_flows::assert_collection_flow(&mut workspace)?;
    guide_flows::assert_localization_id_flow(&mut workspace)?;
    guide_flows::assert_behaviour_check_flow(&mut workspace)?;
    guide_flows::assert_profile_metadata_flow(&mut workspace)?;
    migration_flows::assert_migration_flows(&mut workspace)?;

    example_flows::assert_error_flow(&mut workspace)?;
    example_flows::assert_console_flows(&mut workspace)
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
