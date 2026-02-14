//! Tests for merge helpers in subcommand flows.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use super::util::{with_merged_subcommand_cli, with_merged_subcommand_cli_for};

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
    let cli = MergeArgs {
        foo: Some("cli".into()),
        bar: None,
    };
    let merged: MergeArgs = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
            Ok(())
        },
        &cli,
    )
    .map_err(|err| anyhow!(err))?;
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
    let cli = MergePrefixed { foo: None };
    let merged = with_merged_subcommand_cli_for(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
            Ok(())
        },
        &cli,
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        merged.foo.as_deref() == Some("file"),
        "expected file, got {:?}",
        merged.foo
    );
    Ok(())
}
