//! CLI precedence and required-value behaviour tests.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{MapEnv, SubcommandFileContext, load_and_merge_subcommand_with_sources_at};
use rstest::{fixture, rstest};
use serde::Deserialize;
use std::sync::Arc;

use super::fixtures::{IsolatedRoot, isolated_root};

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq)]
#[command(name = "test")]
struct RequiredCli {
    #[arg(long, required = true)]
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq)]
#[command(name = "test")]
struct OptionalCli {
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<String>,
}

#[fixture]
fn cli_ref_id() -> RequiredCli {
    RequiredCli {
        ref_id: Some("cli".into()),
    }
}

/// Confirms parsed CLI values merge when no file or environment value exists.
#[rstest]
fn cli_only_values_are_accepted(
    cli_ref_id: RequiredCli,
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let merged = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &cli_ref_id,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge CLI-only injected defaults")?;
    ensure!(
        merged.ref_id.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.ref_id
    );
    Ok(())
}

#[test]
fn error_when_required_cli_value_missing() {
    let result = RequiredCli::try_parse_from(["test"]);
    assert!(
        result.is_err(),
        "parsing should fail without required value"
    );
}

#[rstest]
fn conflicting_values_cli_takes_precedence(
    cli_ref_id: RequiredCli,
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open conflicting-values fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nref_id = \"config\"")
        .context("write conflicting-values fixture")?;
    let merged = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &cli_ref_id,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new().with_var("APP_CMDS_TEST_REF_ID", "env")),
    )
    .context("merge conflicting injected defaults")?;
    ensure!(
        merged.ref_id.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.ref_id
    );
    Ok(())
}

/// Confirms an injected environment value fills a missing optional CLI value.
#[rstest]
fn env_value_used_when_cli_missing(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let cli = OptionalCli { ref_id: None };
    let merged = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &cli,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new().with_var("APP_CMDS_TEST_REF_ID", "from-env")),
    )
    .context("merge injected environment defaults")?;
    ensure!(
        merged.ref_id.as_deref() == Some("from-env"),
        "expected from-env, got {:?}",
        merged.ref_id
    );
    Ok(())
}
