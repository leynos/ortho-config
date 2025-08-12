use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, subcommand::load_and_merge_subcommand_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Run(RunArgs),
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default)]
#[command(name = "run")]
struct RunArgs {
    #[arg(long)]
    option: Option<String>,
}

#[test]
fn merge_works_for_subcommand() {
    figment::Jail::expect_with(|j| {
        j.create_file("config.toml", "[cmds.run]\noption = \"file\"")?;
        let cli = Cli::parse_from(["prog", "run", "--option", "cli"]);
        let Commands::Run(args) = cli.cmd;
        let cfg = load_and_merge_subcommand_for(&args)
            .map_err(|e| figment::error::Error::from(e.to_string()))?;
        assert_eq!(cfg.option.as_deref(), Some("cli"));
        Ok(())
    });
}

#[test]
fn merge_falls_back_to_env_when_cli_none() {
    figment::Jail::expect_with(|j| {
        j.set_env("CMDS_RUN_OPTION", "env");
        let cli = Cli::parse_from(["prog", "run"]);
        let Commands::Run(args) = cli.cmd;
        let cfg = load_and_merge_subcommand_for(&args)
            .map_err(|e| figment::error::Error::from(e.to_string()))?;
        assert_eq!(cfg.option.as_deref(), Some("env"));
        Ok(())
    });
}
