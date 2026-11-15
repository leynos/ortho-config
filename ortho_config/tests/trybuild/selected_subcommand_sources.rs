//! Trybuild fixture for the generated selected-subcommand source API.

use std::{path::Path, sync::Arc};

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::{
    MapEnv, OrthoConfig, SelectedSubcommandMerge, SharedScanEnvSource, SubcommandFileContext,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, ortho_config::SelectedSubcommandMerge)]
enum Commands {
    #[command(name = "run")]
    Run(RunArgs),
    #[command(name = "greet")]
    #[ortho_subcommand(with_matches)]
    Greet(GreetArgs),
}

#[derive(Debug, Default, Deserialize, Serialize, Parser, OrthoConfig)]
#[command(name = "run")]
#[ortho_config(prefix = "APP_")]
struct RunArgs {
    #[arg(long)]
    level: Option<u8>,
}

#[derive(Debug, Default, Deserialize, Serialize, Parser, OrthoConfig)]
#[command(name = "greet")]
#[ortho_config(prefix = "APP_")]
struct GreetArgs {
    #[arg(long, default_value_t = default_punctuation())]
    #[ortho_config(cli_default_as_absent)]
    punctuation: String,
}

/// Supplies a clap default so the `with_matches` variant has a CLI extractor.
fn default_punctuation() -> String {
    String::from("!")
}

/// Runs both generated source-aware selected-subcommand paths.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    merge_selected(["prog", "run"])?;
    merge_selected(["prog", "greet"])?;
    Ok(())
}

/// Compiles and exercises the generated merge method for one selected command.
fn merge_selected(arguments: [&str; 2]) -> Result<(), Box<dyn std::error::Error>> {
    let matches = Cli::command().try_get_matches_from(arguments)?;
    let cli = Cli::from_arg_matches(&matches)?;
    let discovery = MapEnv::new();
    let files = SubcommandFileContext::new(
        Path::new("tests/trybuild/selected-subcommand"),
        &discovery,
    );
    let merge_source: SharedScanEnvSource = Arc::new(MapEnv::new());

    cli.command
        .load_and_merge_selected_with_sources(&matches, files, merge_source)?;
    Ok(())
}
