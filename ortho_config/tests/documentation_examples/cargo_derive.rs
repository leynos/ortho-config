//! Run-time contracts for the derive-based Cargo subcommand templates.
//!
//! Both public documents publish the same wrapper. Building each exact fence
//! as `cargo-demo` proves the source compiles, and the probes show that Cargo's
//! injected token reaches the existing options struct.

use anyhow::{Context, Result};
use googletest::prelude::{assert_that, contains_substring, eq};
use pretty_assertions::assert_eq;
use rstest::rstest;
use std::process::Output;

use crate::documentation_examples::documented_example;
use crate::workspace::{DependencyAlias, EnvironmentVariable, ExampleId, ExampleWorkspace};

/// The installed binary name that gives Cargo its `cargo-demo` dispatch
/// target.
const BINARY: &str = "cargo-demo";

/// Both published templates accept Cargo dispatch and direct invocation.
#[rstest]
#[case("readme-cargo-derive-subcommand")]
#[case("guide-cargo-derive-subcommand")]
#[serial_test::serial(documentation_build)]
fn documented_derive_cargo_subcommand(#[case] example: &str) -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    workspace.add_binary_as(BINARY, documented_example(example)?)?;
    workspace.build()?;

    assert_direct_invocation(&mut workspace)?;
    assert_cargo_dispatch(&mut workspace)?;
    assert_help_forms(&mut workspace)?;
    assert_rejected_forms(&mut workspace)
}

/// A documented example paired with the dependency recipe it is published with.
struct PublishedExample {
    recipe: &'static str,
    example: &'static str,
}

/// Each published example builds from its published dependency recipe.
///
/// The generated documentation manifest is not used here, so a recipe that
/// omits a required dependency or package setting fails the build.
#[rstest]
#[case(PublishedExample {
    recipe: "readme-cargo-derive-dependencies",
    example: "readme-cargo-derive-subcommand",
})]
#[case(PublishedExample {
    recipe: "guide-cargo-derive-dependencies",
    example: "guide-cargo-derive-subcommand",
})]
#[case(PublishedExample {
    recipe: "guide-cargo-derive-dependencies",
    example: "guide-cargo-derive-merge",
})]
#[serial_test::serial(documentation_build)]
fn published_recipe_builds_its_example(#[case] pair: PublishedExample) -> Result<()> {
    let mut workspace = ExampleWorkspace::from_recipe(documented_example(pair.recipe)?)?;
    workspace.add_main(documented_example(pair.example)?)?;
    workspace.build()?;

    assert_verbose(
        workspace.run(ExampleId(BINARY), ["demo", "--verbose"])?,
        true,
    )
}

/// The complete adaptation merges environment values beneath the CLI layer.
///
/// An omitted `--verbose` must not contribute clap's `false` default, so the
/// environment value survives; an explicit flag still wins over it.
#[rstest]
#[serial_test::serial(documentation_build)]
fn documented_derive_cargo_subcommand_merges_configuration() -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    workspace.add_binary_as(BINARY, documented_example("guide-cargo-derive-merge")?)?;
    workspace.build()?;

    assert_verbose(workspace.run(ExampleId(BINARY), ["demo"])?, false)?;
    assert_verbose(
        workspace.run_with_environment(
            ExampleId(BINARY),
            ["demo"],
            [EnvironmentVariable {
                name: "CARGO_DEMO_CMDS_DEMO_VERBOSE",
                value: "true",
            }],
        )?,
        true,
    )?;
    assert_verbose(
        workspace.run_with_environment(
            ExampleId(BINARY),
            ["demo", "--verbose"],
            [EnvironmentVariable {
                name: "CARGO_DEMO_CMDS_DEMO_VERBOSE",
                value: "false",
            }],
        )?,
        true,
    )?;
    assert_verbose(
        workspace.run_cargo_subcommand("demo", std::iter::empty::<&str>())?,
        false,
    )
}

/// `cargo-demo demo [--verbose]` reaches the options struct in both forms.
fn assert_direct_invocation(workspace: &mut ExampleWorkspace) -> Result<()> {
    assert_verbose(
        workspace.run(ExampleId(BINARY), ["demo", "--verbose"])?,
        true,
    )?;
    assert_verbose(workspace.run(ExampleId(BINARY), ["demo"])?, false)
}

/// `cargo demo [--verbose]` passes the injected token through to the parser.
fn assert_cargo_dispatch(workspace: &mut ExampleWorkspace) -> Result<()> {
    assert_verbose(workspace.run_cargo_subcommand("demo", ["--verbose"])?, true)?;
    assert_verbose(
        workspace.run_cargo_subcommand("demo", std::iter::empty::<&str>())?,
        false,
    )
}

/// The dispatch event records the parsed option value, not merely success.
fn assert_verbose(output: Output, expected: bool) -> Result<()> {
    assert_that!(
        output.status.code(),
        eq(Some(0)),
        "the invocation should succeed: {output:?}"
    );
    let stderr = String::from_utf8(output.stderr).context("the debug event should be UTF-8")?;
    let observed = debug_field(&stderr, "verbose")
        .with_context(|| format!("the dispatch event should record verbose:\n{stderr}"))?;
    let expected_field = if expected { "true" } else { "false" };
    assert_eq!(observed, expected_field);
    Ok(())
}

/// Extract a `tracing` field value from the deterministic debug output.
fn debug_field<'a>(stderr: &'a str, field: &str) -> Option<&'a str> {
    stderr.lines().find_map(|line| {
        let (_, value) = line.split_once(&format!("{field}="))?;
        value.split_whitespace().next()
    })
}

/// Root help and inner help agree whether Cargo or the user invokes them.
fn assert_help_forms(workspace: &mut ExampleWorkspace) -> Result<()> {
    let root = workspace.run(ExampleId(BINARY), ["--help"])?;
    assert_that!(root.status.code(), eq(Some(0)), "root help should succeed");
    let root_stdout = String::from_utf8(root.stdout).context("root help should be UTF-8")?;
    assert_that!(
        root_stdout,
        contains_substring("Usage: cargo-demo <COMMAND>")
    );
    assert_that!(root_stdout, contains_substring("Commands:"));
    assert_that!(root_stdout, contains_substring("\n  demo"));

    let direct = workspace.run(ExampleId(BINARY), ["demo", "--help"])?;
    assert_that!(
        direct.status.code(),
        eq(Some(0)),
        "direct inner help should succeed"
    );
    let direct_stdout = String::from_utf8(direct.stdout).context("inner help should be UTF-8")?;
    assert_that!(
        direct_stdout,
        contains_substring("Usage: cargo-demo demo [OPTIONS]")
    );
    assert_inner_help(&direct_stdout);

    let dispatched = workspace.run_cargo_subcommand("demo", ["--help"])?;
    assert_that!(
        dispatched.status.code(),
        eq(Some(0)),
        "Cargo-dispatched help should succeed"
    );
    let dispatched_stdout =
        String::from_utf8(dispatched.stdout).context("dispatched help should be UTF-8")?;
    assert_that!(
        dispatched_stdout,
        contains_substring("Usage: cargo-demo demo [OPTIONS]")
    );
    assert_inner_help(&dispatched_stdout);
    Ok(())
}

/// Inner help names the documented option and its description.
fn assert_inner_help(help: &str) {
    assert_that!(help, contains_substring("--verbose"));
    assert_that!(help, contains_substring("Print additional diagnostics"));
}

/// Flat and malformed invocations fail before any application logic runs.
fn assert_rejected_forms(workspace: &mut ExampleWorkspace) -> Result<()> {
    for (subject, args) in [
        ("a missing subcommand", &[][..]),
        ("an unknown subcommand", &["other"][..]),
        ("a flat --verbose option", &["--verbose"][..]),
    ] {
        let output = workspace.run(ExampleId(BINARY), args.iter().copied())?;
        assert_that!(
            output.status.code(),
            eq(Some(2)),
            "{subject} should fail with clap's usage exit code"
        );
        let stderr = String::from_utf8(output.stderr).context("usage errors should be UTF-8")?;
        assert_that!(stderr, contains_substring("Usage: cargo-demo <COMMAND>"));
    }

    let unknown = workspace.run(ExampleId(BINARY), ["demo", "--bogus"])?;
    assert_that!(
        unknown.status.code(),
        eq(Some(2)),
        "an unknown inner option should fail"
    );
    let stderr = String::from_utf8(unknown.stderr).context("usage errors should be UTF-8")?;
    assert_that!(
        stderr,
        contains_substring("Usage: cargo-demo demo [OPTIONS]")
    );
    Ok(())
}
