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

/// Scenario state for Cargo external-subcommand entry-point scenarios.
///
/// Lives with its steps (rather than in the shared `scenario_state` module)
/// to keep this fixture family isolated and the shared module within the
/// repository's module size limit.
#[derive(Default, ScenarioState)]
pub struct CargoContext {
    pub inner_command: Slot<clap::Command>,
    pub installed_binary: Slot<String>,
    pub wrapper: Slot<clap::Command>,
    pub parse_result: Slot<Result<clap::ArgMatches, clap::Error>>,
}

/// Provides a clean Cargo entry-point context for wrapper scenarios.
#[fixture]
pub fn cargo_context() -> CargoContext {
    CargoContext::default()
}

fn flag_long_name(flag: &str) -> Result<String> {
    let flag = normalize_scalar(flag);
    let long_name = flag.trim_start_matches("--");
    ensure!(
        !long_name.is_empty(),
        "flag {flag:?} must carry a long name"
    );
    Ok(long_name.to_owned())
}

#[given("a hand-built clap command named {command_name} with a {flag_name} flag")]
fn build_hand_built_command(
    cargo_context: &CargoContext,
    command_name: String,
    flag_name: String,
) -> Result<()> {
    let command_name = normalize_scalar(&command_name);
    let long_name = flag_long_name(&flag_name)?;
    let command = clap::Command::new(command_name).arg(
        clap::Arg::new(long_name.clone())
            .long(long_name)
            .action(clap::ArgAction::SetTrue),
    );
    cargo_context.inner_command.set(command);
    Ok(())
}

#[when("the command is wrapped for the installed binary {installed_binary}")]
fn wrap_command(cargo_context: &CargoContext, installed_binary: String) -> Result<()> {
    let installed_binary = normalize_scalar(&installed_binary);
    let subcommand_name = installed_binary
        .strip_prefix("cargo-")
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("installed binary {installed_binary:?} lacks the cargo- prefix"))?;
    let inner_command = cargo_context
        .inner_command
        .take()
        .ok_or_else(|| anyhow!("inner command not initialised"))?;
    let wrapper = external_subcommand(installed_binary.clone(), subcommand_name, inner_command);
    cargo_context.installed_binary.set(installed_binary);
    cargo_context.wrapper.set(wrapper);
    Ok(())
}

fn parse_wrapper_arguments(cargo_context: &CargoContext, arguments: &str) -> Result<()> {
    let arguments = normalize_scalar(arguments);
    let installed_binary = cargo_context
        .installed_binary
        .with_ref(Clone::clone)
        .ok_or_else(|| anyhow!("wrapper not initialised"))?;
    let mut argv = vec![installed_binary];
    argv.extend(arguments.split_whitespace().map(str::to_owned));
    let wrapper = cargo_context
        .wrapper
        .take()
        .ok_or_else(|| anyhow!("wrapper not initialised"))?;
    let result = wrapper.try_get_matches_from(argv);
    cargo_context.parse_result.set(result);
    Ok(())
}

#[when("the wrapper parses the Cargo-injected arguments {arguments}")]
fn parse_cargo_injected_arguments(cargo_context: &CargoContext, arguments: String) -> Result<()> {
    parse_wrapper_arguments(cargo_context, &arguments)
}

#[when("the wrapper parses the arguments {arguments}")]
fn parse_bare_arguments(cargo_context: &CargoContext, arguments: String) -> Result<()> {
    parse_wrapper_arguments(cargo_context, &arguments)
}

#[then("parsing succeeds and the {subcommand_name} subcommand sees {flag_name}")]
fn parsing_succeeds_and_flag_is_visible(
    cargo_context: &CargoContext,
    subcommand_name: String,
    flag_name: String,
) -> Result<()> {
    let subcommand_name = normalize_scalar(&subcommand_name);
    let long_name = flag_long_name(&flag_name)?;
    let result = cargo_context
        .parse_result
        .take()
        .ok_or_else(|| anyhow!("parse result not captured"))?;
    let matches = result.map_err(|error| anyhow!("parsing failed: {error}"))?;
    let subcommand_matches = matches
        .subcommand_matches(&subcommand_name)
        .ok_or_else(|| anyhow!("subcommand {subcommand_name:?} missing from parse result"))?;
    ensure!(
        subcommand_matches.get_flag(&long_name),
        "expected the {subcommand_name:?} subcommand to see {flag_name:?}"
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
