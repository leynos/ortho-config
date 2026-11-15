//! Trybuild fixture: the subcommand merge public contracts, used from outside.
//!
//! Deliberately reaches every new public item: `SubcmdConfigMerge`'s four
//! methods, the `SubcommandFileContext` and `SubcommandCliMatches` inputs, and
//! the four injected `_at` loaders. A bound that stops being satisfiable from a
//! downstream call site fails to compile here rather than in every consumer.
//!
//! Both fields are supplied explicitly on the command line, so the merged values
//! must equal the parsed CLI values whatever the ambient directory and
//! environment hold. That is what keeps this fixture's assertions independent of
//! the host: the `_at` loaders are additionally given an empty directory and a
//! closed environment, while the four trait methods resolve their files and
//! variables through the process by design.

use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::subcommand::Prefix;
use ortho_config::{
    MapEnv, OrthoConfig, SharedScanEnvSource, SubcmdConfigMerge, SubcommandCliMatches,
    SubcommandFileContext, load_and_merge_subcommand_for_with_matches_with_sources_at,
    load_and_merge_subcommand_for_with_sources_at,
    load_and_merge_subcommand_with_matches_with_sources_at,
    load_and_merge_subcommand_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::TempDir;

/// The prefix every case uses, chosen so no real application can collide.
const PREFIX: &str = "TRYBUILD_CONTRACT_";

/// Subcommand that satisfies both the blanket impl and the method bounds.
///
/// The `cli_default_as_absent` field is what makes the derive emit
/// `CliValueExtractor`; without it the two match-aware methods would not be
/// callable at all. `Default` is written by hand so the struct default agrees
/// with clap's own.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(name = "trybuild-merge")]
#[ortho_config(prefix = "TRYBUILD_CONTRACT_")]
struct TrybuildMergeArgs {
    /// Explicit `--level`, absent when the user does not pass it.
    #[arg(long)]
    level: Option<u32>,

    /// Carries a clap default, so the derive emits `CliValueExtractor`.
    #[arg(long, default_value_t = String::from("!"))]
    #[ortho_config(cli_default_as_absent)]
    punctuation: String,
}

impl Default for TrybuildMergeArgs {
    /// Mirrors the clap default so both defaults agree.
    fn default() -> Self {
        Self {
            level: None,
            punctuation: String::from("!"),
        }
    }
}

/// Build a closed discovery source naming only `base` for every search root.
///
/// With `HOME`, `XDG_CONFIG_HOME` and `XDG_CONFIG_DIRS` all pointing at an
/// empty directory, discovery has no host path left to consult.
fn closed_discovery(base: &TempDir) -> MapEnv {
    MapEnv::new()
        .with_var("HOME", base.path())
        .with_var("XDG_CONFIG_HOME", base.path())
        .with_var("XDG_CONFIG_DIRS", base.path())
}

/// Assert that `merged` carries exactly the values the caller supplied.
fn assert_cli_values(merged: &TrybuildMergeArgs) {
    assert_eq!(merged.level, Some(7), "explicit --level must survive");
    assert_eq!(
        merged.punctuation, "?",
        "explicit --punctuation must survive"
    );
}

fn main() -> Result<()> {
    let matches = TrybuildMergeArgs::command()
        .try_get_matches_from(["trybuild-merge", "--level", "7", "--punctuation", "?"])
        .context("parse contract CLI")?;
    let cli = TrybuildMergeArgs::from_arg_matches(&matches).context("read contract CLI")?;

    let base = tempfile::tempdir().context("create contract config dir")?;
    let discovery = closed_discovery(&base);
    let scan: SharedScanEnvSource = Arc::new(MapEnv::new());
    let prefix = Prefix::new(PREFIX);

    // The four injected `_at` loaders, each under a name the crate exports.
    let with_sources = load_and_merge_subcommand_with_sources_at(
        &prefix,
        &cli,
        SubcommandFileContext::new(base.path(), &discovery),
        Arc::clone(&scan),
    )
    .context("merge with sources")?;
    let for_with_sources = load_and_merge_subcommand_for_with_sources_at(
        &cli,
        SubcommandFileContext::new(base.path(), &discovery),
        Arc::clone(&scan),
    )
    .context("merge for with sources")?;
    let cli_matches = SubcommandCliMatches::new(&cli, &matches);
    let matched_with_sources = load_and_merge_subcommand_with_matches_with_sources_at(
        &prefix,
        &cli_matches,
        SubcommandFileContext::new(base.path(), &discovery),
        Arc::clone(&scan),
    )
    .context("merge matches with sources")?;
    let for_matched_with_sources = load_and_merge_subcommand_for_with_matches_with_sources_at(
        &cli_matches,
        SubcommandFileContext::new(base.path(), &discovery),
        Arc::clone(&scan),
    )
    .context("merge for matches with sources")?;

    // The four `SubcmdConfigMerge` methods.
    let merged = cli.load_and_merge().context("merge method")?;
    let merged_with_sources = cli
        .load_and_merge_with_sources(Arc::clone(&scan))
        .context("merge method with sources")?;
    let merged_with_matches = cli
        .load_and_merge_with_matches(&matches)
        .context("merge method with matches")?;
    let merged_with_matches_with_sources = cli
        .load_and_merge_with_matches_with_sources(&matches, Arc::clone(&scan))
        .context("merge method with matches and sources")?;

    for outcome in [
        &with_sources,
        &for_with_sources,
        &matched_with_sources,
        &for_matched_with_sources,
        &merged,
        &merged_with_sources,
        &merged_with_matches,
        &merged_with_matches_with_sources,
    ] {
        assert_cli_values(outcome);
    }

    Ok(())
}
