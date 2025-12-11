//! Hello World example entry-point: load config, build greeting plan, print message.

use ortho_config::{SubcmdConfigMerge, is_display_request};

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
    let globals = load_global_config(&cli.globals, cli.config_path.as_deref(), &program)?;
    match cli.command {
        Commands::Greet(args) => {
            // Use load_and_merge_with_matches to respect cli_default_as_absent.
            // This allows [cmds.greet] file config to take precedence over clap
            // defaults when the user doesn't explicitly provide --punctuation.
            let subcommand_matches = CommandLine::greet_matches(&matches)
                .ok_or(HelloWorldError::MissingSubcommandMatches("greet"))?;
            let merged = args.load_and_merge_with_matches(subcommand_matches)?;
            let plan = build_plan(&globals, &merged)?;
            print_plan(&plan)?;
        }
        Commands::TakeLeave(args) => {
            let merged = args.load_and_merge()?;
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
