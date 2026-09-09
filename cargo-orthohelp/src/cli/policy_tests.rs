//! Tests for the agent-native policy check CLI arguments.

use clap::{Parser, error::ErrorKind};
use rstest::rstest;

use super::{CargoSubcommand, Cli};

use crate::policy::PolicyMode;

#[rstest]
fn parses_check_agent_native_flag() {
    let cli = Cli::parse_from(["cargo-orthohelp", "orthohelp", "--check-agent-native"]);
    let CargoSubcommand::Orthohelp(args) = cli.command;

    assert!(args.check_agent_native);
}

#[rstest]
#[case("off", PolicyMode::Off)]
#[case("warn", PolicyMode::Warn)]
#[case("deny", PolicyMode::Deny)]
fn parses_policy_mode_override(#[case] wire: &str, #[case] expected: PolicyMode) {
    let cli = Cli::parse_from([
        "cargo-orthohelp",
        "orthohelp",
        "--check-agent-native",
        "--policy-mode",
        wire,
    ]);
    let CargoSubcommand::Orthohelp(args) = cli.command;

    assert_eq!(args.policy_mode, Some(expected));
}

#[rstest]
fn rejects_policy_mode_without_check_agent_native() {
    let error = Cli::try_parse_from(["cargo-orthohelp", "orthohelp", "--policy-mode", "warn"])
        .expect_err("--policy-mode should require --check-agent-native");

    assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
}

#[rstest]
fn defaults_are_off_and_unset() {
    let cli = Cli::parse_from(["cargo-orthohelp", "orthohelp"]);
    let CargoSubcommand::Orthohelp(args) = cli.command;

    assert!(!args.check_agent_native);
    assert_eq!(args.policy_mode, None);
}
