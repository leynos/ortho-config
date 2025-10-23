//! Shared fixtures and utilities for CLI behaviour tests.

use crate::cli::{
    CommandLine, Commands, FarewellChannel, GreetCommand, HelloWorldCli, TakeLeaveCommand,
};
use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use ortho_config::figment;
use rstest::fixture;

pub type CommandAssertion<'a> = &'a dyn Fn(CommandLine) -> Result<()>;

pub type HelloWorldCliFixture = Result<HelloWorldCli>;
pub type GreetCommandFixture = Result<GreetCommand>;
pub type TakeLeaveCommandFixture = Result<TakeLeaveCommand>;

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

pub fn assert_greet_command(cli: CommandLine) -> Result<()> {
    ensure!(cli.config_path.is_none(), "unexpected config path override");
    ensure!(
        cli.globals.recipient.as_deref() == Some("Crew"),
        "unexpected recipient: {:?}",
        cli.globals.recipient
    );
    ensure!(
        cli.globals.salutations == vec![String::from("Hi")],
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
    ensure!(cli.config_path.is_none(), "unexpected config path override");
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

pub fn figment_error<E: ToString>(err: &E) -> figment::Error {
    figment::Error::from(err.to_string())
}

pub fn with_jail<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<T>,
{
    let mut output = None;
    figment::Jail::try_with(|j| {
        output = Some(f(j)?);
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    output.ok_or_else(|| anyhow!("jail closure did not return a value"))
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
