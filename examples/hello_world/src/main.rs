//! Hello World example entry-point: load config, build greeting plan, print message.

use ortho_config::{
    LoadGlobalsAndSelectedSubcommandError, SelectedSubcommandMergeError, is_display_request,
    load_globals_and_merge_selected_subcommand,
};

use hello_world::cli::{CommandLine, Commands, ParsedCommandLine, load_global_config};
use hello_world::error::{HelloWorldError, Result};
use hello_world::localizer::DemoLocalizer;
use hello_world::message::{build_plan, build_take_leave_plan, print_plan, print_take_leave};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    run().map_err(color_eyre::eyre::Report::from)
}

fn run() -> Result<()> {
    let ParsedCommandLine { cli, matches } = parse_command_line()?;
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
    }
    Ok(())
}

fn parse_command_line() -> Result<ParsedCommandLine> {
    let localizer = DemoLocalizer::default();
    match CommandLine::try_parse_localized_with_matches_env(&localizer) {
        Ok(parsed) => Ok(parsed),
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
