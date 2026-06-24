//! Hello World example entry-point: load config, build greeting plan, print message.

use ortho_config::{
    LoadGlobalsAndSelectedSubcommandError, SelectedSubcommandMergeError, is_display_request,
    load_globals_and_merge_selected_subcommand, parse_localized_command,
};

use clap::CommandFactory;
use hello_world::cli::context::{ContextCommand, context_json_pointer, render_agent_context_json};
use hello_world::cli::{CommandLine, Commands, LocalizeCmd, load_global_config};
use hello_world::error::{HelloWorldError, Result};
use hello_world::localizer::DemoLocalizer;
use hello_world::message::{build_plan, build_take_leave_plan, print_plan, print_take_leave};

use std::io::{self, Write};

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

    match command {
        Commands::Greet(merged) => {
            let plan = build_plan(&globals, &merged)?;
            print_plan(&plan)?;
        }
        Commands::TakeLeave(merged) => {
            let plan = build_take_leave_plan(&globals, &merged)?;
            print_take_leave(&plan)?;
        }
        Commands::Context(context) => print_context(&context)?,
    }
    Ok(())
}

fn print_context(context: &ContextCommand) -> Result<()> {
    let output = if context.json {
        render_agent_context_json().map_err(|err| HelloWorldError::Internal(Box::new(err)))?
    } else {
        context_json_pointer()
    };
    io::stdout().write_all(output.as_bytes())?;
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
