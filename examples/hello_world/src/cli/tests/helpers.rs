//! Shared fixtures and utilities for CLI behaviour tests.

use crate::cli::{
    CommandLine, Commands, FarewellChannel, FileOverrides, GreetCommand, HelloWorldCli,
    TakeLeaveCommand, load_config_overrides,
};
use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use ortho_config::figment;
use rstest::fixture;

pub type CommandAssertion<'a> = &'a dyn Fn(CommandLine) -> Result<()>;

pub type HelloWorldCliFixture = Result<HelloWorldCli>;
pub type GreetCommandFixture = Result<GreetCommand>;
pub type TakeLeaveCommandFixture = Result<TakeLeaveCommand>;

pub use crate::test_support::{figment_error, with_jail};

#[fixture]
pub fn base_cli() -> HelloWorldCliFixture {
    let cli = HelloWorldCli::default();
    ensure!(
        !cli.salutations.is_empty(),
        "default salutations should contain at least one entry"
    );
    Ok(cli)
}

#[fixture]
pub fn greet_command() -> GreetCommandFixture {
    let command = GreetCommand::default();
    ensure!(
        !command.punctuation.trim().is_empty(),
        "default greet punctuation must not be empty"
    );
    Ok(command)
}

#[fixture]
pub fn take_leave_command() -> TakeLeaveCommandFixture {
    let command = TakeLeaveCommand::default();
    ensure!(
        !command.parting.trim().is_empty(),
        "default farewell must not be empty"
    );
    Ok(command)
}

pub fn parse_command_line(args: &[&str]) -> Result<CommandLine> {
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push("hello-world");
    full_args.extend_from_slice(args);
    CommandLine::try_parse_from(full_args).context("parse command line")
}

fn assert_common_cli_prechecks(cli: &CommandLine) -> Result<()> {
    ensure!(cli.config_path.is_none(), "unexpected config path override");
    Ok(())
}

pub fn assert_greet_command(cli: CommandLine) -> Result<()> {
    assert_common_cli_prechecks(&cli)?;
    ensure!(
        cli.globals.recipient.as_deref() == Some("Crew"),
        "unexpected recipient: {:?}",
        cli.globals.recipient
    );
    ensure!(
        cli.globals.salutations == vec!["Hi".to_owned()],
        "unexpected salutations"
    );
    let greet = expect_greet(cli.command)?;
    ensure!(
        greet.preamble.as_deref() == Some("Good morning"),
        "unexpected preamble"
    );
    ensure!(greet.punctuation == "?!", "unexpected punctuation");
    Ok(())
}

pub fn assert_take_leave_command(cli: CommandLine) -> Result<()> {
    assert_common_cli_prechecks(&cli)?;
    ensure!(cli.globals.is_excited, "expected excited global flags");
    let command = expect_take_leave(cli.command)?;
    ensure!(command.parting == "Cheerio", "unexpected parting");
    ensure!(
        command.gift.as_deref() == Some("flowers"),
        "unexpected gift"
    );
    ensure!(command.remind_in == Some(20), "unexpected reminder");
    ensure!(
        command.channel == Some(FarewellChannel::Message),
        "unexpected farewell channel"
    );
    ensure!(command.wave, "expected wave flag");
    Ok(())
}

pub fn expect_greet(command: Commands) -> Result<GreetCommand> {
    match command {
        Commands::Greet(greet) => Ok(greet),
        Commands::TakeLeave(_) => Err(anyhow!("expected greet command, found take-leave")),
    }
}

pub fn expect_take_leave(command: Commands) -> Result<TakeLeaveCommand> {
    match command {
        Commands::TakeLeave(take_leave) => Ok(take_leave),
        Commands::Greet(_) => Err(anyhow!("expected take-leave command, found greet")),
    }
}

pub(crate) fn load_overrides_in_jail<S>(setup: S) -> Result<Option<FileOverrides>>
where
    S: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    with_jail(|j| {
        setup(j)?;
        load_config_overrides()
            .map(|result| result.map(|(overrides, _)| overrides))
            .map_err(figment_error)
    })
}

pub(crate) fn expect_overrides<S>(setup: S) -> Result<FileOverrides>
where
    S: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    load_overrides_in_jail(setup)?.ok_or_else(|| anyhow!("expected overrides"))
}

pub fn assert_sample_greet_defaults(greet: &GreetCommand) -> Result<()> {
    ensure!(
        greet.preamble.as_deref() == Some("Layered hello"),
        "unexpected sample greet preamble: {:?}",
        greet.preamble
    );
    ensure!(
        greet.punctuation == "!!!",
        "unexpected sample punctuation: {}",
        greet.punctuation
    );
    Ok(())
}
