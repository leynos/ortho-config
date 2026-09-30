//! Baseline loading behaviour for subcommand configuration.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{
    MapEnv, OrthoError, SubcommandFileContext, load_and_merge_subcommand_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

use super::fixtures::close_discovery;
use super::to_anyhow::ToAnyhow as _;

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
/// Paths here are nested (`home/.app.toml`, `xdg/app/config.toml`), so the
/// parent directories are created first; `cap_std::fs::Dir::write` opens with
/// `O_CREAT` but never creates intermediate components.
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

/// Load and merge `CmdCfg` from `base`, `discovery`, and `merge_env`.
///
/// Every case in this module loads through the same prefix and the same
/// default CLI value, so those stay here rather than being restated at each
/// call site. What varies — the file base, the discovery source, and the
/// values merged on top — stays with the caller.
///
/// The conversion through [`ToAnyhow`] happens here rather than at each call
/// site so the tests can keep attaching their own scenario-specific
/// `.context(...)` while still reporting through `anyhow`.
fn load_cfg(base: &Path, discovery: &MapEnv, merge_env: MapEnv) -> Result<CmdCfg> {
    load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(base, discovery),
        Arc::new(merge_env),
    )
    .to_anyhow()
}

#[test]
fn file_and_env_loading() -> Result<()> {
    let root = tempfile::tempdir().context("create file and environment fixture")?;
    write_config(
        root.path(),
        Path::new(".app.toml"),
        "[cmds.test]\nfoo = \"file\"\nbar = true",
    )?;
    let discovery = close_discovery(root.path());
    let cfg = load_cfg(
        root.path(),
        &discovery,
        MapEnv::new().with_var("APP_CMDS_TEST_FOO", "env"),
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
    let discovery = close_discovery(root.path()).with_var("HOME", &home);
    let cfg = load_cfg(&base, &discovery, MapEnv::new())
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
    let discovery = close_discovery(root.path()).with_var("HOME", &home);
    let cfg = load_cfg(&base, &discovery, MapEnv::new())
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
    let discovery = close_discovery(root.path()).with_var("XDG_CONFIG_HOME", &xdg);
    let cfg = load_cfg(&base, &discovery, MapEnv::new())
        .context("merge XDG defaults from injected source")?;
    ensure!(
        cfg.foo.as_deref() == Some("xdg"),
        "expected xdg, got {:?}",
        cfg.foo
    );
    Ok(())
}

/// The public loader returns non-missing XDG metadata failures with their path.
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_metadata_error_reaches_public_subcommand_loader() -> Result<()> {
    let root = tempfile::tempdir().context("create XDG error fixture")?;
    write_config(root.path(), Path::new("not-a-directory"), "")?;
    let not_a_directory = root.path().join("not-a-directory");
    let candidate = not_a_directory.join("app/config.toml");
    let discovery = close_discovery(root.path()).with_var("XDG_CONFIG_HOME", &not_a_directory);

    let error = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .expect_err("a failed XDG metadata probe must reach the public loader");
    let OrthoError::File {
        path,
        source: error_source,
    } = error.as_ref()
    else {
        anyhow::bail!("expected a file error, got {error:?}");
    };
    ensure!(
        path == &candidate,
        "reported candidate path differs: {path:?}"
    );
    let io_error = error_source
        .downcast_ref::<std::io::Error>()
        .context("metadata error source was not an I/O error")?;
    ensure!(
        io_error.kind() != std::io::ErrorKind::NotFound,
        "a non-directory parent must not be treated as a missing candidate"
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
    let discovery = close_discovery(root.path());
    let cfg = load_cfg(root.path(), &discovery, MapEnv::new())
        .context("merge YAML defaults from explicit base")?;
    ensure!(
        cfg.foo.as_deref() == Some("yaml"),
        "expected yaml, got {:?}",
        cfg.foo
    );
    Ok(())
}
