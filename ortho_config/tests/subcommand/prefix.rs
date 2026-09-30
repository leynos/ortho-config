//! Prefix-handling tests for subcommand wrappers.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandFileContext, load_and_merge_subcommand_for_with_sources_at,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::fixtures::{IsolatedRoot, isolated_root};

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq)]
#[ortho_config(prefix = "APP_")]
#[command(name = "test")]
struct PrefixedCfg {
    foo: Option<String>,
}

/// Confirms the configured struct prefix is used by the wrapper loader.
#[rstest]
fn wrapper_uses_struct_prefix(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open prefixed wrapper fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"val\"")
        .context("write prefixed wrapper fixture")?;
    let cfg = load_and_merge_subcommand_for_with_sources_at(
        &PrefixedCfg::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new().with_var("APP_CMDS_TEST_FOO", "env")),
    )
    .context("merge prefixed injected defaults")?;
    ensure!(
        cfg.foo.as_deref() == Some("env"),
        "expected env, got {:?}",
        cfg.foo
    );
    Ok(())
}
