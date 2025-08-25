//! Integration tests for subcommand merge behaviour:
//!
//! - CLI values override file/environment.
//! - CLI-provided `None` must not override file/environment values (sanitized provider).
//!
//! Precedence (lowest -> highest): struct defaults < file < environment < CLI.
//! Omitting a CLI flag (yielding `None`) must not shadow values from file/environment;
//! the sanitized provider ignores `None` and preserves the prior source.
//!
//! Example:
//! - Given `.app.toml` with:
//!   `[cmds.run]`
//!   `option = "file"`
//! - And `APP_CMDS_RUN_OPTION=env` in the environment:
//!   - `prog run --option cli` => `option = "cli"` (CLI wins)
//!   - `prog run`              => `option = "env"` (CLI is `None`, environment wins)
//!   - no CLI, no environment  => `option = "file"` (file wins)

use clap::{Parser, Subcommand};
use ortho_config::OrthoConfig;
use ortho_config::subcommand::SubcmdConfigMerge;
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
    #[arg(long)]
    count: Option<u32>,
}

#[test]
fn merge_works_for_subcommand() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.run]\noption = \"file\"")?;
        let cli = Cli::parse_from(["prog", "run", "--option", "cli"]);
        let Commands::Run(args) = cli.cmd;
        let cfg = args.load_and_merge()?;
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
        let cfg = args.load_and_merge()?;
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
        let cfg = args.load_and_merge()?;
        assert_eq!(cfg.option.as_deref(), Some("file"));
        Ok(())
    });
}

#[test]
fn merge_errors_on_invalid_file() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.run]\noption = 5")?;
        let cli = Cli::parse_from(["prog", "run"]);
        let Commands::Run(args) = cli.cmd;
        assert!(args.load_and_merge().is_err());
        Ok(())
    });
}

#[test]
fn merge_errors_on_invalid_env() {
    figment::Jail::expect_with(|j| {
        j.set_env("APP_CMDS_RUN_COUNT", "not-a-number");
        let cli = Cli::parse_from(["prog", "run"]);
        let Commands::Run(args) = cli.cmd;
        assert!(args.load_and_merge().is_err());
        Ok(())
    });
}
