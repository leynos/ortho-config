//! Shared fixtures for discovery integration tests.

use std::path::PathBuf;

use anyhow::{Context, Result};
use rstest::fixture;
use tempfile::TempDir;
use test_helpers::env::{self as test_env, EnvScope};

use super::super::ConfigDiscovery;

fn remove_common_env_vars(env_lock: &test_env::EnvVarLock) -> Vec<test_env::EnvVarGuard> {
    let mut guards = Vec::new();
    for key in [
        "HELLO_WORLD_CONFIG_PATH",
        "XDG_CONFIG_HOME",
        "XDG_CONFIG_DIRS",
        "APPDATA",
        "LOCALAPPDATA",
        "HOME",
        "USERPROFILE",
    ] {
        guards.push(env_lock.remove_var(key));
    }
    guards
}

#[fixture]
pub(super) fn env_guards() -> EnvScope {
    test_env::EnvScope::new_with(remove_common_env_vars)
}

#[fixture]
pub(super) fn env_override_discovery() -> (ConfigDiscovery, PathBuf, EnvScope, test_env::EnvVarGuard)
{
    let scope = test_env::EnvScope::new_with(remove_common_env_vars);
    let path = std::env::temp_dir().join("explicit.toml");
    let env_guard = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &path);
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .build();

    (discovery, path, scope, env_guard)
}

#[fixture]
pub(super) fn config_temp_dir() -> Result<TempDir> {
    TempDir::new().context("create config directory")
}

#[fixture]
pub(super) fn sample_config_file() -> Result<(TempDir, PathBuf)> {
    let temp_dir = config_temp_dir()?;
    let file_dir = temp_dir.path().join("hello_world");
    std::fs::create_dir_all(&file_dir).context("create hello_world directory")?;
    let file = file_dir.join("config.toml");
    std::fs::write(&file, "is_enabled = true").context("write config file")?;
    Ok((temp_dir, file))
}
