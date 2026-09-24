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

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{Parser, Subcommand};
use ortho_config::subcommand::Prefix;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandFileContext, load_and_merge_subcommand_with_sources_at,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Describes a file and injected environment layer for one merge case.
struct SourceSetup {
    file: Option<&'static str>,
    env: Option<(&'static str, &'static str)>,
}

/// Stage a file within a temporary root and merge without process mutation.
fn merge_from_sources<T>(setup: &SourceSetup, args: T) -> Result<RunArgs>
where
    T: IntoIterator,
    T::Item: Into<std::ffi::OsString> + Clone,
{
    let fixture = tempfile::tempdir().context("create subcommand source root")?;
    let directory = Dir::open_ambient_dir(fixture.path(), ambient_authority())
        .context("open subcommand source root")?;
    if let Some(contents) = setup.file {
        directory
            .write(".app.toml", contents.as_bytes())
            .context("write subcommand configuration fixture")?;
    }
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", fixture.path());
    let merge_source = match setup.env {
        Some((key, value)) => MapEnv::new().with_var(key, value),
        None => MapEnv::new(),
    };
    let cli = Cli::try_parse_from(args).context("parse subcommand CLI arguments")?;
    let Commands::Run(values) = cli.cmd;
    load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &values,
        SubcommandFileContext::new(fixture.path(), &discovery),
        Arc::new(merge_source),
    )
    .map_err(|error| anyhow::anyhow!(error))
    .context("merge subcommand source layers")
}

/// A successful merge scenario and the `option` value it should yield.
struct MergeCase {
    setup: SourceSetup,
    cli_args: &'static [&'static str],
    expected: &'static str,
}

#[rstest]
#[case::cli_overrides_file(MergeCase {
    setup: SourceSetup { file: Some(OPTION_FROM_FILE), env: None },
    cli_args: &["prog", "run", "--option", "cli"],
    expected: "cli",
})]
#[case::env_when_cli_none(MergeCase {
    setup: SourceSetup {
        file: Some(OPTION_FROM_FILE),
        env: Some(("APP_CMDS_RUN_OPTION", "env")),
    },
    cli_args: &["prog", "run"],
    expected: "env",
})]
#[case::file_when_cli_none(MergeCase {
    setup: SourceSetup { file: Some(OPTION_FROM_FILE), env: None },
    cli_args: &["prog", "run"],
    expected: "file",
})]
fn merge_resolves_option_from_expected_layer(#[case] case: MergeCase) -> Result<()> {
    let cfg = merge_from_sources(&case.setup, case.cli_args.iter().copied())?;
    ensure!(
        cfg.option.as_deref() == Some(case.expected),
        "expected option {:?} from the staged layer, got {:?}",
        case.expected,
        cfg.option.as_deref()
    );
    Ok(())
}

#[rstest]
#[case::invalid_file(SourceSetup {
    file: Some("[cmds.run]\noption = 5"),
    env: None,
})]
#[case::invalid_env(SourceSetup {
    file: None,
    env: Some(("APP_CMDS_RUN_COUNT", "not-a-number")),
})]
fn merge_errors_on_malformed_source(#[case] setup: SourceSetup) -> Result<()> {
    let result = merge_from_sources(&setup, ["prog", "run"]);
    ensure!(
        result.is_err(),
        "expected the malformed source to fail the merge"
    );
    Ok(())
}
