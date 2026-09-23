//! Prefix-handling tests for subcommand wrappers.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandFileContext, load_and_merge_subcommand_for_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq)]
#[ortho_config(prefix = "APP_")]
#[command(name = "test")]
struct PrefixedCfg {
    foo: Option<String>,
}

#[test]
fn wrapper_uses_struct_prefix() -> Result<()> {
    let root = tempfile::tempdir().context("create prefixed wrapper fixture")?;
    let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
        .context("open prefixed wrapper fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"val\"")
        .context("write prefixed wrapper fixture")?;
    #[cfg(any(unix, target_os = "redox"))]
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    #[cfg(not(any(unix, target_os = "redox")))]
    let discovery = MapEnv::new();
    let cfg = load_and_merge_subcommand_for_with_sources_at(
        &PrefixedCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
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
