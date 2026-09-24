//! Tests for merging selected subcommand enums via `SelectedSubcommandMerge`.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::{
    LoadGlobalsAndSelectedSubcommandError, MapEnv, OrthoConfig, SelectedSubcommandMerge,
    SelectedSubcommandMergeError, SelectedSubcommandSources, SubcommandFileContext,
    load_globals_and_merge_selected_subcommand_with_sources,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    #[ortho_config(cli_default_as_absent)]
    punctuation: String,
}

fn default_punctuation() -> String {
    String::from("!")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Globals {
    value: u8,
}

/// Write one selected-subcommand fixture in a scoped temporary directory.
///
/// These tests use the same file shape but keep their merge assertions at each
/// call site. The returned root stays alive while the selected command loads.
fn selected_file(contents: &str) -> Result<tempfile::TempDir> {
    let root = tempfile::tempdir().context("create selected-subcommand fixture root")?;
    let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
        .context("open selected-subcommand fixture root")?;
    directory
        .write(".app.toml", contents.as_bytes())
        .context("write selected-subcommand configuration")?;
    Ok(root)
}

#[rstest]
fn selected_subcommand_merge_respects_cli_default_as_absent() -> Result<()> {
    let root = selected_file("[cmds.greet]\npunctuation = \"??\"")?;
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let matches = Cli::command()
        .try_get_matches_from(["prog", "greet"])
        .context("parse greet arguments")?;
    let cli = Cli::from_arg_matches(&matches).context("decode greet arguments")?;
    let merged =
        cli.cmd
            .load_and_merge_selected_with_sources(&matches, files, Arc::new(MapEnv::new()))?;
    let Commands::Greet(cfg) = merged else {
        anyhow::bail!("expected greet command");
    };
    ensure!(
        cfg.punctuation == "??",
        "file should override the clap default"
    );
    Ok(())
}

#[rstest]
fn unified_helper_returns_globals_and_merged_command() -> Result<()> {
    let root = selected_file("[cmds.run]\noption = \"file\"")?;
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let matches = Cli::command()
        .try_get_matches_from(["prog", "run"])
        .context("parse run arguments")?;
    let cli = Cli::from_arg_matches(&matches).context("decode run arguments")?;
    let (globals, merged) = load_globals_and_merge_selected_subcommand_with_sources(
        &matches,
        cli.cmd,
        SelectedSubcommandSources::new(files, Arc::new(MapEnv::new())),
        || Ok::<_, std::io::Error>(Globals { value: 7 }),
    )?;
    ensure!(
        globals == Globals { value: 7 },
        "globals should be retained"
    );
    let Commands::Run(cfg) = merged else {
        anyhow::bail!("expected run command");
    };
    ensure!(
        cfg.option.as_deref() == Some("file"),
        "file value should merge"
    );
    Ok(())
}

#[rstest]
fn selected_subcommand_merge_uses_injected_environment() -> Result<()> {
    let root = selected_file("# no command defaults\n")?;
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let matches = Cli::command()
        .try_get_matches_from(["prog", "run"])
        .context("parse run arguments")?;
    let cli = Cli::from_arg_matches(&matches).context("decode run arguments")?;
    let environment = Arc::new(MapEnv::new().with_var("APP_CMDS_RUN_OPTION", "injected"));
    let merged = cli
        .cmd
        .load_and_merge_selected_with_sources(&matches, files, environment)?;

    let Commands::Run(config) = merged else {
        anyhow::bail!("expected run command");
    };
    ensure!(
        config.option.as_deref() == Some("injected"),
        "selected merge should read the injected environment"
    );
    Ok(())
}

#[rstest]
fn selected_subcommand_merge_errors_when_missing_subcommand_matches() {
    let matches = clap::ArgMatches::default();

    let selected_command = Commands::Greet(GreetArgs::default());
    let err = selected_command
        .load_and_merge_selected(&matches)
        .expect_err("expected missing subcommand matches error");

    assert!(
        matches!(
            err,
            SelectedSubcommandMergeError::MissingSubcommandMatches {
                selected: selected_name
            } if selected_name == "greet"
        ),
        "expected MissingSubcommandMatches for greet, got {err:?}"
    );
}

#[rstest]
fn unified_helper_surfaces_globals_error() -> Result<()> {
    let root = selected_file("# no command defaults\n")?;
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let matches = Cli::command()
        .try_get_matches_from(["prog", "run"])
        .context("parse run arguments")?;
    let cli = Cli::from_arg_matches(&matches).context("decode run arguments")?;
    let error = load_globals_and_merge_selected_subcommand_with_sources(
        &matches,
        cli.cmd,
        SelectedSubcommandSources::new(files, Arc::new(MapEnv::new())),
        || Err::<Globals, std::io::Error>(std::io::Error::other("boom")),
    );
    ensure!(
        matches!(
            error,
            Err(LoadGlobalsAndSelectedSubcommandError::Globals(_))
        ),
        "expected global loading failure: {error:?}"
    );
    Ok(())
}

#[rstest]
fn selected_subcommand_merge_surfaces_merge_error() -> Result<()> {
    let root = selected_file("[cmds.greet]\npunctuation = 123")?;
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let matches = Cli::command()
        .try_get_matches_from(["prog", "greet"])
        .context("parse greet arguments")?;
    let cli = Cli::from_arg_matches(&matches).context("decode greet arguments")?;
    let error =
        cli.cmd
            .load_and_merge_selected_with_sources(&matches, files, Arc::new(MapEnv::new()));
    ensure!(
        matches!(error, Err(SelectedSubcommandMergeError::Merge(_))),
        "expected selected-subcommand merge failure: {error:?}"
    );
    Ok(())
}
