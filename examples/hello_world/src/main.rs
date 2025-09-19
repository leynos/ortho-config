mod cli;
mod error;
mod message;

use crate::cli::HelloWorldCli;
use crate::error::HelloWorldError;
use crate::message::{build_plan, print_plan};
use ortho_config::OrthoConfig;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    run().map_err(color_eyre::eyre::Report::from)
}

fn run() -> Result<(), HelloWorldError> {
    let cli = HelloWorldCli::load()?;
    let plan = build_plan(&cli)?;
    print_plan(&plan);
    Ok(())
}
