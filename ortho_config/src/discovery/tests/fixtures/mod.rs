//! Shared fixtures for discovery integration tests.

use std::io::Write as _;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir as Utf8Dir};
use rstest::fixture;
use tempfile::TempDir;

use super::super::ConfigDiscovery;
use crate::MapEnv;

#[fixture]
pub(super) fn env_override_discovery() -> Result<(ConfigDiscovery, Utf8PathBuf)> {
    let temp_dir = std::env::temp_dir();
    let utf8_temp_dir = Utf8PathBuf::from_path_buf(temp_dir)
        .map_err(|path| anyhow!("temporary directory path is not valid UTF-8: {path:?}"))?;
    let path = utf8_temp_dir.join("explicit.toml");
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(
            MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", path.as_std_path()),
        ))
        .build();

    Ok((discovery, path))
}

#[fixture]
pub(super) fn config_temp_dir() -> Result<TempDir> {
    TempDir::new().context("create config directory")
}

#[fixture]
pub(super) fn sample_config_file() -> Result<(TempDir, Utf8PathBuf)> {
    let temp_dir = config_temp_dir()?;
    let utf8_temp_dir = Utf8Path::from_path(temp_dir.path()).ok_or_else(|| {
        anyhow!(
            "temporary directory path is not valid UTF-8: {:?}",
            temp_dir.path()
        )
    })?;
    let cap_temp_dir = Utf8Dir::open_ambient_dir(utf8_temp_dir, ambient_authority())
        .context("open temporary directory with cap-std")?;
    cap_temp_dir
        .create_dir("hello_world")
        .context("create hello_world directory")?;
    let hello_world_dir = cap_temp_dir
        .open_dir("hello_world")
        .context("open hello_world directory")?;
    let mut config_file = hello_world_dir
        .create("config.toml")
        .context("create config file")?;
    config_file
        .write_all(b"is_enabled = true")
        .context("write config file")?;
    let file = utf8_temp_dir.join("hello_world/config.toml");
    Ok((temp_dir, file))
}
