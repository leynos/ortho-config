//! Hello World example entry-point: load config, build greeting plan, print message.

use clap::Parser;
use ortho_config::{SubcmdConfigMerge, is_display_request};

use hello_world::cli::{CommandLine, Commands, apply_greet_overrides, load_global_config};
use hello_world::error::HelloWorldError;
use hello_world::message::{build_plan, build_take_leave_plan, print_plan, print_take_leave};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    run().map_err(color_eyre::eyre::Report::from)
}

fn run() -> Result<(), HelloWorldError> {
    let cli = parse_command_line()?;
    let globals = load_global_config(&cli.globals, cli.config_path.as_deref())?;
    match cli.command {
        Commands::Greet(args) => {
            let mut merged = args.load_and_merge()?;
            apply_greet_overrides(&mut merged)?;
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

fn parse_command_line() -> Result<CommandLine, HelloWorldError> {
    match CommandLine::try_parse() {
        Ok(cli) => Ok(cli),
        Err(err) => {
            if is_display_request(&err) {
                err.exit();
            }
            Err(err.into())
        }
    }
}
