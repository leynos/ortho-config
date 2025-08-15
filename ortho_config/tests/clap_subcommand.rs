//! Integration tests for subcommand merge behaviour:
//! - CLI values override file/env.
//! - CLI None must not override file/env values (sanitized provider).
//!
//! Precedence (lowest -> highest): struct defaults < file < env < CLI.
//! Omitting a CLI flag (yielding `None`) must not shadow values from file/env;
//! the sanitized provider ignores `None` and preserves the prior source.

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
#[ortho_config(prefix = "APP_")]
struct RunArgs {
    #[arg(long)]
    option: Option<String>,
}

#[test]
fn merge_works_for_subcommand() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.run]\noption = \"file\"")?;
        let cli = Cli::parse_from(["prog", "run", "--option", "cli"]);
        let Commands::Run(args) = cli.cmd;
        let cfg = load_and_merge_subcommand_for(&args)?;
        assert_eq!(cfg.option.as_deref(), Some("cli"));
        Ok(())
    });
}

#[test]
fn merge_falls_back_to_env_when_cli_none() {
    figment::Jail::expect_with(|j| {
        j.set_env("APP_CMDS_RUN_OPTION", "env");
        let cli = Cli::parse_from(["prog", "run"]);
        let Commands::Run(args) = cli.cmd;
        let cfg = load_and_merge_subcommand_for(&args)?;
        assert_eq!(cfg.option.as_deref(), Some("env"));
        Ok(())
    });
}

#[test]
fn merge_falls_back_to_file_when_cli_none() {
    figment::Jail::expect_with(|j| {
        // Strip all env vars so file fallback is deterministic.
        j.clear_env();
        j.create_file(".app.toml", "[cmds.run]\noption = \"file\"")?;
        let cli = Cli::parse_from(["prog", "run"]);
        let Commands::Run(args) = cli.cmd;
        let cfg = load_and_merge_subcommand_for(&args)?;
        assert_eq!(cfg.option.as_deref(), Some("file"));
        Ok(())
    });
}
