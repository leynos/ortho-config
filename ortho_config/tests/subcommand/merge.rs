//! Tests for merge helpers in subcommand flows.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandFileContext, load_and_merge_subcommand_for_with_sources_at,
    load_and_merge_subcommand_with_sources_at,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::fixtures::{IsolatedRoot, isolated_root};

#[derive(Debug, Deserialize, Serialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct MergeArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bar: Option<bool>,
}

/// Tests that merging CLI arguments with configuration file values prioritizes CLI values and preserves unset fields.
///
/// This test creates a configuration file with a default value for `foo`, then merges it with CLI arguments that override `foo` and leave `bar` unset. It asserts that the merged configuration uses the CLI value for `foo` and retains `None` for `bar`.
///
/// # Examples
///
/// ```
/// merge_helper_combines_defaults_and_cli();
/// ```
#[rstest]
fn merge_helper_combines_defaults_and_cli(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open merge helper fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"file\"")
        .context("write merge helper fixture")?;
    let cli = MergeArgs {
        foo: Some("cli".into()),
        bar: None,
    };
    let merged = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &cli,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge explicit CLI and file defaults")?;
    ensure!(
        merged.foo.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.foo
    );
    ensure!(merged.bar.is_none(), "expected None, got {:?}", merged.bar);
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default, PartialEq, Parser)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergePrefixed {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

/// Verifies that `MergePrefixed` respects the configuration prefix and
/// prefers file values when the CLI field is unset.
#[rstest]
fn merge_wrapper_respects_prefix(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open prefixed merge fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"file\"")
        .context("write prefixed merge fixture")?;
    let cli = MergePrefixed { foo: None };
    let merged = load_and_merge_subcommand_for_with_sources_at(
        &cli,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge prefixed explicit file defaults")?;
    ensure!(
        merged.foo.as_deref() == Some("file"),
        "expected file, got {:?}",
        merged.foo
    );
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default, PartialEq, Parser)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergeNonFinite {
    #[serde(skip_serializing_if = "Option::is_none")]
    ratio: Option<f64>,
}

/// A configured non-finite float is data, not absence.
///
/// The requirement enumeration projects the merged figment into JSON, which
/// cannot represent `nan` or `inf`, so the projection sees `null`. Reading the
/// final value back out of that projection would report `None` for a value the
/// user really configured, and would fail outright for a plain `f64`. This
/// case therefore pins the extraction to the original figment.
#[rstest]
fn a_non_finite_float_survives_the_schema_aware_subcommand_merge(
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open non-finite merge fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nratio = nan")
        .context("write non-finite merge fixture")?;
    let merged = load_and_merge_subcommand_for_with_sources_at(
        &MergeNonFinite::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge a configured non-finite value")?;
    ensure!(
        merged.ratio.is_some_and(|ratio| ratio.is_nan()),
        "a configured nan must survive the merge, got {:?}",
        merged.ratio
    );
    Ok(())
}
