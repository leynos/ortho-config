//! Regressions for required-candidate traversal and selector anchoring.
//!
//! These cases complement `loading.rs` without pushing it past the
//! repository's 400-line file cap. They exercise two fail-closed properties
//! that a first-success walk can otherwise hide: a required candidate after the
//! first success, and a relative selector the environment source cannot anchor.

use std::sync::Arc;

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;

use super::super::fixtures::{config_temp_dir, sample_config_file};
use super::{expect_file_error, open_cap_utf8_dir, write_cap_file};
use crate::MapEnv;
use crate::discovery::ConfigDiscovery;

/// A source holding fixture variables but unable to report a working directory.
#[derive(Debug)]
struct NoWorkingDirectory(MapEnv);

impl crate::EnvSource for NoWorkingDirectory {
    fn get(&self, key: &str) -> Option<std::ffi::OsString> {
        crate::EnvSource::get(&self.0, key)
    }

    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        Err(std::io::Error::other("deleted"))
    }
}

/// A required candidate after the first success is still checked.
///
/// Both paths are required and precede every optional candidate, so the first
/// load is the first required path. The walk must still attempt the second
/// required path; otherwise its absence is hidden by the first success.
#[rstest]
fn load_first_checks_required_candidates_after_a_success(
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let first = dir_path.join("first.toml");
    let missing = dir_path.join("second-missing.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "first.toml", "is_enabled = true")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
        .add_required_path(&first)
        .add_required_path(&missing)
        .build();

    let outcome = discovery.load_first_partitioned();
    ensure!(
        outcome.figment.is_some(),
        "expected the first required candidate to load"
    );
    ensure!(
        outcome.required_errors.len() == 1,
        "expected the missing second required path, got {} error(s)",
        outcome.required_errors.len()
    );
    expect_file_error(discovery.load_first(), "second-missing.toml")
}

/// A relative selector that cannot be anchored fails closed.
///
/// The automatic XDG candidate would load, so the only reason for the failure
/// is the selector's required resolution error. Resolving the path against the
/// process working directory instead would be the boundary violation
/// `ConfigDiscovery` exists to prevent.
#[rstest]
fn load_first_fails_when_relative_selector_cannot_be_anchored(
    sample_config_file: Result<(TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp_dir, _config_path) = sample_config_file?;
    let source = NoWorkingDirectory(
        MapEnv::new()
            .with_var("XDG_CONFIG_HOME", temp_dir.path())
            .with_var("HELLO_WORLD_CONFIG_PATH", "chosen.toml"),
    );
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(source))
        .build();

    let outcome = discovery.load_first_partitioned();
    ensure!(
        outcome.figment.is_some(),
        "expected the automatic candidate to load"
    );
    ensure!(
        !outcome.required_errors.is_empty(),
        "expected the unanchorable selector to be a required error"
    );
    expect_file_error(discovery.load_first(), "chosen.toml")
}
