//! Snapshot tests for the Cargo external-subcommand entry-point wrapper.
//!
//! Pins the load-bearing renderings of [`ortho_config::cargo::external_subcommand`]:
//! the top-level usage line, the subcommand usage line, and the
//! zero-argument error a confused user sees first. Whole help screens are
//! deliberately not snapshotted because they drift across clap patch
//! releases; only the lines that document the Cargo dispatch shape are kept.

use clap::error::ErrorKind;
use insta::assert_snapshot;
use ortho_config::cargo::external_subcommand;

fn demo_command() -> clap::Command {
    clap::Command::new("demo").version("1.2.3").arg(
        clap::Arg::new("verbose")
            .long("verbose")
            .action(clap::ArgAction::SetTrue),
    )
}

fn wrapped_demo() -> clap::Command {
    external_subcommand("cargo-demo", "demo", demo_command())
}

fn usage_lines(rendered: &str) -> String {
    rendered
        .lines()
        .filter(|line| line.starts_with("Usage:"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn top_level_help_renders_cargo_dispatch_usage() {
    let mut wrapper = wrapped_demo();
    let help = wrapper.render_help().to_string();

    assert!(
        help.contains("Usage: cargo <COMMAND>"),
        "unexpected top-level help:\n{help}"
    );
    assert_snapshot!("top_level_help_usage", usage_lines(&help));
}

#[test]
fn subcommand_help_renders_cargo_dispatch_usage() {
    let error = wrapped_demo()
        .try_get_matches_from(["cargo-demo", "demo", "--help"])
        .expect_err("help should short-circuit parsing");
    assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    let help = error.to_string();

    assert!(
        help.contains("Usage: cargo demo [OPTIONS]"),
        "unexpected subcommand help:\n{help}"
    );
    assert_snapshot!("subcommand_help_usage", usage_lines(&help));
}

#[test]
fn zero_argument_invocation_renders_missing_subcommand_error() {
    let error = wrapped_demo()
        .try_get_matches_from(["cargo-demo"])
        .expect_err("bare invocation should fail");
    assert_eq!(error.kind(), ErrorKind::MissingSubcommand);
    let rendered = error.to_string();

    assert!(
        rendered.contains("Usage: cargo <COMMAND>"),
        "unexpected error rendering:\n{rendered}"
    );
    assert_snapshot!("zero_argument_error", rendered);
}
