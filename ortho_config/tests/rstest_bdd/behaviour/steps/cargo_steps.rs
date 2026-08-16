//! Step definitions for Cargo external-subcommand entry-point scenarios.
//!
//! These steps drive [`ortho_config::cargo::external_subcommand`] from a
//! consumer's point of view: wrapping a hand-built `clap::Command` for the
//! installed binary, parsing both the Cargo-injected argument form and the
//! bare form, and observing the inner options one level down under the
//! injected subcommand name.

use super::value_parsing::normalize_scalar;
use anyhow::{Result, anyhow, ensure};
use clap::error::ErrorKind;
use ortho_config::cargo::external_subcommand;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};
use std::str::FromStr;

#[derive(Debug)]
struct CommandName(String);

#[derive(Debug)]
struct LongFlagName(String);

#[derive(Debug, Clone)]
struct InstalledBinaryName(String);

#[derive(Debug)]
struct CargoSubcommandName(String);

#[derive(Debug)]
struct CargoArguments(Vec<String>);

impl FromStr for CommandName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(normalize_scalar(value)))
    }
}

impl FromStr for LongFlagName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let flag = normalize_scalar(value);
        let long_name = flag.trim_start_matches("--");
        ensure!(
            !long_name.is_empty(),
            "flag {flag:?} must carry a long name"
        );
        Ok(Self(long_name.to_owned()))
    }
}

impl FromStr for InstalledBinaryName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let installed_binary = normalize_scalar(value);
        ensure!(
            installed_binary.starts_with("cargo-"),
            "installed binary {installed_binary:?} lacks the cargo- prefix"
        );
        ensure!(
            installed_binary != "cargo-",
            "installed binary {installed_binary:?} has an empty subcommand"
        );
        ensure!(
            installed_binary != "cargo-help",
            "installed binary {installed_binary:?} uses clap's reserved help subcommand"
        );
        Ok(Self(installed_binary))
    }
}

impl FromStr for CargoSubcommandName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(normalize_scalar(value)))
    }
}

impl From<&InstalledBinaryName> for CargoSubcommandName {
    fn from(installed_binary: &InstalledBinaryName) -> Self {
        Self(installed_binary.0.replacen("cargo-", "", 1))
    }
}

impl FromStr for CargoArguments {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            normalize_scalar(value)
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        ))
    }
}

/// Scenario state for Cargo external-subcommand entry-point scenarios.
///
/// Lives with its steps (rather than in the shared `scenario_state` module)
/// to keep this fixture family isolated and the shared module within the
/// repository's module size limit.
#[derive(Default, ScenarioState)]
pub struct CargoContext {
    pub inner_command: Slot<clap::Command>,
    installed_binary: Slot<InstalledBinaryName>,
    pub wrapper: Slot<clap::Command>,
    pub parse_result: Slot<Result<clap::ArgMatches, clap::Error>>,
}

/// Provides a clean Cargo entry-point context for wrapper scenarios.
#[fixture]
pub fn cargo_context() -> CargoContext {
    CargoContext::default()
}

#[given("a hand-built clap command named {command_name} with a {flag_name} flag")]
fn build_hand_built_command(
    cargo_context: &CargoContext,
    command_name: CommandName,
    flag_name: LongFlagName,
) -> Result<()> {
    let command = clap::Command::new(command_name.0).arg(
        clap::Arg::new(flag_name.0.clone())
            .long(flag_name.0)
            .action(clap::ArgAction::SetTrue),
    );
    cargo_context.inner_command.set(command);
    Ok(())
}

#[when("the command is wrapped for the installed binary {installed_binary}")]
fn wrap_command(cargo_context: &CargoContext, installed_binary: InstalledBinaryName) -> Result<()> {
    let subcommand_name = CargoSubcommandName::from(&installed_binary);
    let inner_command = cargo_context
        .inner_command
        .take()
        .ok_or_else(|| anyhow!("inner command not initialized"))?;
    let wrapper = external_subcommand(installed_binary.0.clone(), subcommand_name.0, inner_command);
    cargo_context.installed_binary.set(installed_binary);
    cargo_context.wrapper.set(wrapper);
    Ok(())
}

fn parse_wrapper_arguments(cargo_context: &CargoContext, arguments: CargoArguments) -> Result<()> {
    let installed_binary = cargo_context
        .installed_binary
        .with_ref(Clone::clone)
        .ok_or_else(|| anyhow!("wrapper not initialized"))?;
    let mut argv = vec![installed_binary.0];
    argv.extend(arguments.0);
    let wrapper = cargo_context
        .wrapper
        .take()
        .ok_or_else(|| anyhow!("wrapper not initialized"))?;
    let result = wrapper.try_get_matches_from(argv);
    cargo_context.parse_result.set(result);
    Ok(())
}

#[when("the wrapper parses the Cargo-injected arguments {arguments}")]
fn parse_cargo_injected_arguments(
    cargo_context: &CargoContext,
    arguments: CargoArguments,
) -> Result<()> {
    parse_wrapper_arguments(cargo_context, arguments)
}

#[when("the wrapper parses the arguments {arguments}")]
fn parse_bare_arguments(cargo_context: &CargoContext, arguments: CargoArguments) -> Result<()> {
    parse_wrapper_arguments(cargo_context, arguments)
}

#[then("parsing succeeds and the {subcommand_name} subcommand sees {flag_name}")]
fn parsing_succeeds_and_flag_is_visible(
    cargo_context: &CargoContext,
    subcommand_name: CargoSubcommandName,
    flag_name: LongFlagName,
) -> Result<()> {
    let result = cargo_context
        .parse_result
        .take()
        .ok_or_else(|| anyhow!("parse result not captured"))?;
    let matches = result.map_err(|error| anyhow!("parsing failed: {error}"))?;
    let subcommand_matches = matches
        .subcommand_matches(&subcommand_name.0)
        .ok_or_else(|| {
            anyhow!(
                "subcommand {:?} missing from parse result",
                subcommand_name.0
            )
        })?;
    ensure!(
        subcommand_matches.get_flag(&flag_name.0),
        "expected the {:?} subcommand to see {:?}",
        subcommand_name.0,
        flag_name.0
    );
    Ok(())
}

#[then("parsing fails because the subcommand token is missing")]
fn parsing_fails_without_injected_token(cargo_context: &CargoContext) -> Result<()> {
    let result = cargo_context
        .parse_result
        .take()
        .ok_or_else(|| anyhow!("parse result not captured"))?;
    let error = result
        .err()
        .ok_or_else(|| anyhow!("parsing unexpectedly succeeded"))?;
    ensure!(
        matches!(
            error.kind(),
            ErrorKind::UnknownArgument
                | ErrorKind::InvalidSubcommand
                | ErrorKind::MissingSubcommand
        ),
        "unexpected error kind {:?}: {error}",
        error.kind()
    );
    Ok(())
}
