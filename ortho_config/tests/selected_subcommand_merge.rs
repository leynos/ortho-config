//! Tests for merging selected subcommand enums via `SelectedSubcommandMerge`.

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::{
    OrthoConfig, SelectedSubcommandMerge, load_globals_and_merge_selected_subcommand,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand, ortho_config_macros::SelectedSubcommandMerge)]
enum Commands {
    #[command(name = "run")]
    Run(RunArgs),
    #[command(name = "greet")]
    #[ortho_subcommand(with_matches)]
    Greet(GreetArgs),
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq, Eq)]
#[command(name = "run")]
#[ortho_config(prefix = "APP_")]
struct RunArgs {
    #[arg(long)]
    option: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq, Eq)]
#[command(name = "greet")]
#[ortho_config(prefix = "APP_")]
struct GreetArgs {
    #[arg(long, default_value_t = default_punctuation())]
    #[ortho_config(default = default_punctuation(), cli_default_as_absent)]
    punctuation: String,
}

fn default_punctuation() -> String {
    String::from("!")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Globals {
    value: u8,
}

#[rstest]
fn selected_subcommand_merge_respects_cli_default_as_absent() {
    figment::Jail::expect_with(|jail| {
        jail.create_file(".app.toml", "[cmds.greet]\npunctuation = \"??\"")?;

        let command = Cli::command();
        let matches = command
            .try_get_matches_from(["prog", "greet"])
            .expect("expected clap parsing to succeed");
        let cli = Cli::from_arg_matches(&matches).expect("expected clap decoding to succeed");

        let merged = cli
            .cmd
            .load_and_merge_selected(&matches)
            .map_err(|err| figment::Error::from(err.to_string()))?;
        let Commands::Greet(cfg) = merged else {
            panic!("expected greet command");
        };
        assert_eq!(cfg.punctuation, "??");
        Ok(())
    });
}

#[rstest]
fn unified_helper_returns_globals_and_merged_command() {
    figment::Jail::expect_with(|jail| {
        jail.create_file(".app.toml", "[cmds.run]\noption = \"file\"")?;

        let command = Cli::command();
        let matches = command
            .try_get_matches_from(["prog", "run"])
            .expect("expected clap parsing to succeed");
        let cli = Cli::from_arg_matches(&matches).expect("expected clap decoding to succeed");

        let (globals, merged) =
            load_globals_and_merge_selected_subcommand(&matches, cli.cmd, || {
                Ok::<_, std::io::Error>(Globals { value: 7 })
            })
            .map_err(|err| figment::Error::from(err.to_string()))?;

        assert_eq!(globals, Globals { value: 7 });
        let Commands::Run(cfg) = merged else {
            panic!("expected run command");
        };
        assert_eq!(cfg.option.as_deref(), Some("file"));
        Ok(())
    });
}
