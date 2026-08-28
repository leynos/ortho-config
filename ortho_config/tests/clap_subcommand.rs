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
use ortho_config::{OrthoConfig, ResultIntoFigment, SubcmdConfigMerge};
use rstest::rstest;
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

/// Configuration file contents supplying `option` from the file layer.
const OPTION_FROM_FILE: &str = "[cmds.run]\noption = \"file\"";

/// Describes the sources a jailed test should stage before merging.
struct JailSetup {
    /// Contents written to `.app.toml`, when the file layer participates.
    file: Option<&'static str>,
    /// Environment variable name and value, when the environment participates.
    env: Option<(&'static str, &'static str)>,
    /// Strips the inherited environment so file fallback stays deterministic.
    clear_env: bool,
}

impl JailSetup {
    /// Stages the described sources inside `jail`.
    ///
    /// Boxes the error to keep the `Err` variant small for
    /// `clippy::result_large_err`; the jail closure unboxes on propagation.
    /// The process environment is cleared first when `clear_env` is `true`,
    /// then `.app.toml` is written when `file` is `Some`, then the named
    /// variable is set when `env` is `Some`.
    ///
    /// # Examples
    /// ```
    /// figment::Jail::expect_with(|jail| {
    ///     let setup = JailSetup {
    ///         file: Some("[cmds.run]\noption = \"file\""),
    ///         env: None,
    ///         clear_env: true,
    ///     };
    ///     setup.apply(jail).map_err(|error| *error)?;
    ///     Ok(())
    /// });
    /// ```
    fn apply(&self, jail: &mut figment::Jail) -> Result<(), Box<figment::Error>> {
        if self.clear_env {
            jail.clear_env();
        }
        if let Some(contents) = self.file {
            jail.create_file(".app.toml", contents)?;
        }
        if let Some((key, value)) = self.env {
            jail.set_env(key, value);
        }
        Ok(())
    }
}

/// A successful merge scenario and the `option` value it should yield.
struct MergeCase {
    setup: JailSetup,
    cli_args: &'static [&'static str],
    expected: &'static str,
}

#[rstest]
#[case::cli_overrides_file(MergeCase {
    setup: JailSetup { file: Some(OPTION_FROM_FILE), env: None, clear_env: false },
    cli_args: &["prog", "run", "--option", "cli"],
    expected: "cli",
})]
#[case::env_when_cli_none(MergeCase {
    setup: JailSetup {
        file: Some(OPTION_FROM_FILE),
        env: Some(("APP_CMDS_RUN_OPTION", "env")),
        clear_env: false,
    },
    cli_args: &["prog", "run"],
    expected: "env",
})]
#[case::file_when_cli_none(MergeCase {
    setup: JailSetup { file: Some(OPTION_FROM_FILE), env: None, clear_env: true },
    cli_args: &["prog", "run"],
    expected: "file",
})]
fn merge_resolves_option_from_expected_layer(#[case] case: MergeCase) -> anyhow::Result<()> {
    figment::Jail::try_with(|j| {
        case.setup.apply(j).map_err(|error| *error)?;
        let cli = Cli::parse_from(case.cli_args.iter().copied());
        let Commands::Run(args) = cli.cmd;
        let cfg = args.load_and_merge().to_figment()?;
        if cfg.option.as_deref() != Some(case.expected) {
            return Err(figment::Error::from(format!(
                "expected option {:?} from the staged layer, got {:?}",
                case.expected,
                cfg.option.as_deref()
            )));
        }
        Ok(())
    })?;
    Ok(())
}

#[rstest]
#[case::invalid_file(JailSetup {
    file: Some("[cmds.run]\noption = 5"),
    env: None,
    clear_env: false,
})]
#[case::invalid_env(JailSetup {
    file: None,
    env: Some(("APP_CMDS_RUN_COUNT", "not-a-number")),
    clear_env: false,
})]
fn merge_errors_on_malformed_source(#[case] setup: JailSetup) -> anyhow::Result<()> {
    figment::Jail::try_with(|j| {
        setup.apply(j).map_err(|error| *error)?;
        let cli = Cli::parse_from(["prog", "run"]);
        let Commands::Run(args) = cli.cmd;
        if args.load_and_merge().is_ok() {
            return Err(figment::Error::from(
                "expected the malformed source to fail the merge".to_owned(),
            ));
        }
        Ok(())
    })?;
    Ok(())
}
