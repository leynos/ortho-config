//! Baseline loading behaviour for subcommand configuration.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{MapEnv, SubcommandFileContext, load_and_merge_subcommand_with_sources_at};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct CmdCfg {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    foo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    bar: Option<bool>,
}

/// Write one fixture below `root` through an explicitly scoped capability.
///
/// This stays private to the baseline-loading module: its callers need the
/// same simple file shape, while the other subcommand suites exercise distinct
/// configuration boundaries and should keep those arrangements explicit.
fn write_config(root: &Path, relative: &Path, contents: &str) -> Result<()> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .context("open basic subcommand fixture directory")?;
    if let Some(parent) = relative
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        directory
            .create_dir_all(parent)
            .context("create basic subcommand fixture parent")?;
    }
    directory
        .write(relative, contents.as_bytes())
        .context("write basic subcommand fixture")?;
    Ok(())
}

/// Return a closed discovery source with the Unix global rung pinned to `root`.
#[cfg(any(unix, target_os = "redox"))]
fn isolated_discovery(root: &Path) -> MapEnv {
    MapEnv::new().with_var("XDG_CONFIG_DIRS", root)
}

/// Return a closed discovery source on non-Unix and non-Redox targets.
#[cfg(not(any(unix, target_os = "redox")))]
fn isolated_discovery(_root: &Path) -> MapEnv {
    MapEnv::new()
}

#[test]
fn file_and_env_loading() -> Result<()> {
    let root = tempfile::tempdir().context("create file and environment fixture")?;
    write_config(
        root.path(),
        Path::new(".app.toml"),
        "[cmds.test]\nfoo = \"file\"\nbar = true",
    )?;
    let discovery = isolated_discovery(root.path());
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
        Arc::new(MapEnv::new().with_var("APP_CMDS_TEST_FOO", "env")),
    )
    .context("merge file and injected environment defaults")?;
    ensure!(
        cfg.foo.as_deref() == Some("env"),
        "expected env, got {:?}",
        cfg.foo
    );
    ensure!(cfg.bar == Some(true), "expected true, got {:?}", cfg.bar);
    Ok(())
}

#[test]
fn loads_from_home() -> Result<()> {
    let root = tempfile::tempdir().context("create home fixture")?;
    let home = root.path().join("home");
    let base = root.path().join("base");
    write_config(
        root.path(),
        Path::new("home/.app.toml"),
        "[cmds.test]\nfoo = \"home\"",
    )?;
    let discovery = isolated_discovery(root.path()).with_var("HOME", &home);
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(&base, &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge home defaults from injected source")?;
    ensure!(
        cfg.foo.as_deref() == Some("home"),
        "expected home, got {:?}",
        cfg.foo
    );
    Ok(())
}

#[test]
fn local_overrides_home() -> Result<()> {
    let root = tempfile::tempdir().context("create local-overrides-home fixture")?;
    let home = root.path().join("home");
    let base = root.path().join("base");
    write_config(
        root.path(),
        Path::new("home/.app.toml"),
        "[cmds.test]\nfoo = \"home\"",
    )?;
    write_config(
        root.path(),
        Path::new("base/.app.toml"),
        "[cmds.test]\nfoo = \"local\"",
    )?;
    let discovery = isolated_discovery(root.path()).with_var("HOME", &home);
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(&base, &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge local defaults after injected home defaults")?;
    ensure!(
        cfg.foo.as_deref() == Some("local"),
        "expected local, got {:?}",
        cfg.foo
    );
    Ok(())
}

// Windows lacks XDG support.
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() -> Result<()> {
    let root = tempfile::tempdir().context("create XDG fixture")?;
    let xdg = root.path().join("xdg");
    let base = root.path().join("base");
    write_config(
        root.path(),
        Path::new("xdg/app/config.toml"),
        "[cmds.test]\nfoo = \"xdg\"",
    )?;
    let discovery = isolated_discovery(root.path()).with_var("XDG_CONFIG_HOME", &xdg);
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(&base, &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge XDG defaults from injected source")?;
    ensure!(
        cfg.foo.as_deref() == Some("xdg"),
        "expected xdg, got {:?}",
        cfg.foo
    );
    Ok(())
}

#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() -> Result<()> {
    let root = tempfile::tempdir().context("create YAML fixture")?;
    write_config(
        root.path(),
        Path::new(".app.yml"),
        "cmds:\n  test:\n    foo: yaml",
    )?;
    let discovery = isolated_discovery(root.path());
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge YAML defaults from explicit base")?;
    ensure!(
        cfg.foo.as_deref() == Some("yaml"),
        "expected yaml, got {:?}",
        cfg.foo
    );
    Ok(())
}
