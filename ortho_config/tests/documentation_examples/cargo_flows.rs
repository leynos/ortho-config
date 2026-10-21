//! Run-time contracts for the Cargo external-subcommand examples.
//!
//! These examples need their own workspace: Cargo dispatch looks binaries up
//! by their `cargo-<name>` file name, so the parser example is built under the
//! dispatchable name `cargo-demo` rather than its example identifier.

use anyhow::{Result, ensure};

use crate::documentation_examples::documented_example;
use crate::workspace::{DependencyAlias, ExampleId, ExampleWorkspace};

#[test]
#[serial_test::serial(documentation_build)]
fn documented_cargo_external_subcommand_parses_both_invocation_forms() -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    workspace.add_binary_as(
        "cargo-demo",
        documented_example("guide-cargo-external-subcommand")?,
    )?;
    workspace.add_binary(documented_example("guide-cargo-error-hints")?)?;
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

    // The guide documents the version-less parent as a deliberate obligation.
    let parent_version = workspace.run(ExampleId("cargo-demo"), ["--version"])?;
    ensure!(
        !parent_version.status.success(),
        "the synthetic parent should not print a version"
    );

    assert_error_hints(&mut workspace)
}

/// The hint program appends the invocation note to the two Cargo-shaped
/// parse failures and leaves successful parses alone.
fn assert_error_hints(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = "guide-cargo-error-hints";
    const NOTE: &str =
        "note: invoke this tool via `cargo <name>` or as `cargo-<name> <name> [OPTIONS]`";
    for args in [&[][..], &["demo", "--bogus"][..]] {
        let output = workspace.run(ExampleId(ID), args.iter())?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        ensure!(
            output.status.code() == Some(2) && stderr.contains(NOTE),
            "{args:?} should fail with clap's usage exit code and the invocation note:\n{stderr}"
        );
    }
    let accepted = workspace.run(ExampleId(ID), ["demo"])?;
    ensure!(
        accepted.status.success() && accepted.stderr.is_empty(),
        "a well-formed invocation should succeed without a note"
    );
    Ok(())
}
