//! Hello World example entry-point: load config, build greeting plan, print message.

use ortho_config::{
    LoadGlobalsAndSelectedSubcommandError, SelectedSubcommandMergeError, is_display_request,
    load_globals_and_merge_selected_subcommand, parse_localized_command,
};

use clap::CommandFactory;
use hello_world::cli::context::{ContextCommand, context_json_pointer, render_agent_context_json};
use hello_world::cli::{
    CommandLine, Commands, GreetCommand, HelloWorldCli, LocalizeCmd, TakeLeaveCommand,
    load_global_config,
};
use hello_world::error::{HelloWorldError, Result};
use hello_world::localizer::DemoLocalizer;
use hello_world::message::{build_plan, build_take_leave_plan, print_plan, print_take_leave};

use std::io::{self, Write};
use tracing::{debug, error};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    run().map_err(color_eyre::eyre::Report::from)
}

struct ParsedCommandLine {
    cli: CommandLine,
    matches: clap::ArgMatches,
}
fn run() -> Result<()> {
    let ParsedCommandLine { cli, matches } = parse_command_line()?;
    if let Commands::Context(context) = &cli.command {
        debug!(
            command = "context",
            json = context.json,
            "dispatching agent context"
        );
        return print_context(context);
    }

    let program = std::env::args_os()
        .next()
        .unwrap_or_else(|| std::ffi::OsString::from("hello-world"));

    let (globals, command) =
        load_globals_and_merge_selected_subcommand(&matches, cli.command, || {
            load_global_config(&cli.globals, cli.config_path.as_deref(), &program)
        })
        .map_err(map_load_error)?;

    execute_command(&globals, command)
}

fn print_context(context: &ContextCommand) -> Result<()> {
    let output = if context.json {
        render_agent_context_json().map_err(|err| {
            error!(command = "context", json = true, error = %err, "agent context serialization failed");
            HelloWorldError::Internal(Box::new(err))
        })?
    } else {
        context_json_pointer()
    };
    io::stdout().write_all(output.as_bytes()).map_err(|err| {
        error!(command = "context", json = context.json, error = %err, "agent context output failed");
        HelloWorldError::Output(err)
    })?;
    Ok(())
}

/// Dispatch the selected subcommand against the merged global configuration.
fn execute_command(globals: &HelloWorldCli, command: Commands) -> Result<()> {
    match command {
        Commands::Greet(merged) => run_greet(globals, &merged),
        Commands::TakeLeave(merged) => run_take_leave(globals, &merged),
        #[expect(
            clippy::unreachable,
            reason = "the early return in run handles context before configuration merging"
        )]
        Commands::Context(_) => {
            unreachable!("context commands return before configuration merging")
        }
    }
}

/// Build and print the greeting plan for the `greet` subcommand.
fn run_greet(globals: &HelloWorldCli, merged: &GreetCommand) -> Result<()> {
    let plan = build_plan(globals, merged)?;
    print_plan(&plan)?;
    Ok(())
}

/// Build and print the farewell plan for the `take-leave` subcommand.
fn run_take_leave(globals: &HelloWorldCli, merged: &TakeLeaveCommand) -> Result<()> {
    let plan = build_take_leave_plan(globals, merged)?;
    print_take_leave(&plan)?;
    Ok(())
}

fn parse_command_line() -> Result<ParsedCommandLine> {
    let localizer = DemoLocalizer::default();
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(&localizer);

    match parse_localized_command::<CommandLine, _, _>(command, std::env::args_os(), &localizer) {
        Ok((cli, matches)) => Ok(ParsedCommandLine { cli, matches }),
        Err(err) => {
            if is_display_request(&err) {
                err.exit();
            }
            Err(err.into())
        }
    }
}

fn map_load_error(
    load_err: LoadGlobalsAndSelectedSubcommandError<HelloWorldError>,
) -> HelloWorldError {
    match load_err {
        LoadGlobalsAndSelectedSubcommandError::Globals(globals_err) => globals_err,
        LoadGlobalsAndSelectedSubcommandError::Subcommand(subcommand_err) => {
            map_selected_subcommand_error(subcommand_err)
        }
        other => HelloWorldError::Internal(Box::new(other)),
    }
}

fn map_selected_subcommand_error(error: SelectedSubcommandMergeError) -> HelloWorldError {
    match error {
        SelectedSubcommandMergeError::MissingSubcommandMatches { selected } => {
            HelloWorldError::MissingSubcommandMatches(selected)
        }
        SelectedSubcommandMergeError::Merge(merge_err) => HelloWorldError::Configuration(merge_err),
        other => HelloWorldError::Internal(Box::new(other)),
    }
}
