//! Unit tests for XDG base resolution and candidate selection.
//!
//! Every case here drives discovery through a closed [`MapEnv`], so the
//! ordering and first-existing rules are asserted without reading the host's
//! environment. Cases that need an injected home fallback or platform
//! configuration root live in the sibling `tests` module instead, because they
//! depend on an `EnvSource` implementation that only those tests need.
//!
//! The module is Unix-only: it is compiled solely under
//! `cfg(any(unix, target_os = "redox"))`, matching the platform that provides
//! the XDG rules it exercises.

use super::*;
use crate::MapEnv;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A closed map never inherits the host's XDG configuration home.
#[test]
fn closed_map_omits_host_xdg_home() {
    let bases = source_xdg_bases(&Prefix::new("app"), &MapEnv::new());
    assert_eq!(bases, vec![PathBuf::from("/etc/xdg/app")]);
}

/// An empty or wholly relative `XDG_CONFIG_DIRS` falls back to `/etc/xdg`.
#[rstest::rstest]
#[case::empty("")]
#[case::relative("relative")]
fn unusable_xdg_dirs_use_platform_default(#[case] dirs: &str) {
    let bases = source_xdg_bases(
        &Prefix::new("app"),
        &MapEnv::new().with_var("XDG_CONFIG_DIRS", dirs),
    );
    assert_eq!(bases, vec![PathBuf::from("/etc/xdg/app")]);
}

/// Absolute entries keep their listed order; interleaved relative ones drop out.
#[test]
fn xdg_dirs_keep_absolute_entries_in_order() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let first = root.path().join("first");
    let second = root.path().join("second");
    let dirs = std::env::join_paths([first.as_path(), Path::new("relative"), second.as_path()])
        .context("join XDG configuration directories")?;
    let bases = source_xdg_bases(
        &Prefix::new("app"),
        &MapEnv::new().with_var("XDG_CONFIG_DIRS", dirs),
    );
    ensure!(
        bases == [first.join("app"), second.join("app")],
        "absolute XDG directories changed order: {bases:?}"
    );
    Ok(())
}

/// The first XDG directory containing `config.toml` wins even when a later
/// directory also has one.
#[test]
fn xdg_dirs_search_prefers_first_directory_with_config() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(first.join("app")).context("create first XDG directory")?;
    fs::create_dir_all(second.join("app")).context("create second XDG directory")?;
    fs::write(first.join("app/config.toml"), "").context("write first config")?;
    fs::write(second.join("app/config.toml"), "").context("write second config")?;

    let joined = std::env::join_paths([first.as_path(), Path::new("relative"), second.as_path()])
        .context("join XDG configuration directories")?;
    let source = MapEnv::new().with_var("XDG_CONFIG_DIRS", joined);
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source)?;

    let config_candidates: Vec<&PathBuf> = paths
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "config.toml"))
        .collect();
    ensure!(
        config_candidates == vec![&first.join("app/config.toml")],
        "expected exactly the first XDG directory's config.toml, got {config_candidates:?}"
    );
    Ok(())
}

/// A directory without a match is skipped in favour of a later one that has it.
#[test]
fn xdg_dirs_search_falls_through_to_later_directory() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(first.join("app")).context("create first XDG directory")?;
    fs::create_dir_all(second.join("app")).context("create second XDG directory")?;
    fs::write(second.join("app/config.toml"), "").context("write second config")?;

    let joined = std::env::join_paths([first.as_path(), second.as_path()])
        .context("join XDG configuration directories")?;
    let source = MapEnv::new().with_var("XDG_CONFIG_DIRS", joined);
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source)?;

    let config_candidates: Vec<&PathBuf> = paths
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "config.toml"))
        .collect();
    ensure!(
        config_candidates == vec![&second.join("app/config.toml")],
        "expected the later XDG directory's config.toml, got {config_candidates:?}"
    );
    Ok(())
}

/// Any path with metadata counts as existing, matching `xdg` 3's probe.
///
/// The candidate here is a *directory* called `config.toml`; it still counts,
/// because the probe is deliberately metadata-only and not a file test.
#[test]
fn xdg_search_keeps_metadata_existence_contract() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let config = root.path().join("app/config.toml");
    fs::create_dir_all(&config).context("create directory at config path")?;
    let source = MapEnv::new().with_var("XDG_CONFIG_HOME", root.path());
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source)?;
    ensure!(
        paths.contains(&config),
        "XDG 3 treats any path with metadata as an existing candidate"
    );
    Ok(())
}
