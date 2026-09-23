//! Tests for merge helpers in subcommand flows.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandFileContext, load_and_merge_subcommand_for_with_sources_at,
    load_and_merge_subcommand_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
#[test]
fn merge_helper_combines_defaults_and_cli() -> Result<()> {
    let root = tempfile::tempdir().context("create merge helper fixture")?;
    let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
        .context("open merge helper fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"file\"")
        .context("write merge helper fixture")?;
    #[cfg(any(unix, target_os = "redox"))]
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    #[cfg(not(any(unix, target_os = "redox")))]
    let discovery = MapEnv::new();
    let cli = MergeArgs {
        foo: Some("cli".into()),
        bar: None,
    };
    let merged = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &cli,
        SubcommandFileContext::new(root.path(), &discovery),
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
#[test]
fn merge_wrapper_respects_prefix() -> Result<()> {
    let root = tempfile::tempdir().context("create prefixed merge fixture")?;
    let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
        .context("open prefixed merge fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"file\"")
        .context("write prefixed merge fixture")?;
    #[cfg(any(unix, target_os = "redox"))]
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    #[cfg(not(any(unix, target_os = "redox")))]
    let discovery = MapEnv::new();
    let cli = MergePrefixed { foo: None };
    let merged = load_and_merge_subcommand_for_with_sources_at(
        &cli,
        SubcommandFileContext::new(root.path(), &discovery),
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
