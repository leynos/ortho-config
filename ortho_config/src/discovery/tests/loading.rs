//! Loading behaviour tests for discovery.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, ensure};
use rstest::rstest;
use serde::Deserialize;
use tempfile::TempDir;
use test_helpers::env::{self as test_env, EnvScope};

use super::super::*;
use super::fixtures::{config_temp_dir, env_guards, sample_config_file};

#[derive(Debug, Deserialize)]
struct SampleConfig {
    value: bool,
}

#[rstest]
fn load_first_reads_first_existing_file(
    env_guards: EnvScope,
    sample_config_file: Result<(TempDir, PathBuf)>,
) -> Result<()> {
    let _guards = env_guards;
    let (temp_dir, _config_path) = sample_config_file?;
    let _xdg = test_env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    let discovery = ConfigDiscovery::builder("hello_world").build();
    let figment = match discovery.load_first() {
        Ok(Some(figment)) => figment,
        Ok(None) => return Err(anyhow!("expected configuration candidate to load")),
        Err(err) => return Err(anyhow!("discovery failed to load config: {err}")),
    };
    let config: SampleConfig = figment
        .extract()
        .context("extract sample config from figment")?;
    ensure!(config.value, "expected loaded config to set value=true");
    Ok(())
}

#[rstest]
fn load_first_skips_invalid_candidates(
    env_guards: EnvScope,
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let _guards = env_guards;
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let invalid = dir_path.join("broken.toml");
    let valid = dir_path.join("valid.toml");
    std::fs::write(&invalid, "value = ???").context("write invalid config")?;
    std::fs::write(&valid, "value = false").context("write valid config")?;
    let _env = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &invalid);

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .add_explicit_path(valid.clone())
        .build();

    let figment = match discovery.load_first() {
        Ok(Some(figment)) => figment,
        Ok(None) => return Err(anyhow!("expected fallback configuration to load")),
        Err(err) => return Err(anyhow!("discovery failed to load configuration: {err}")),
    };
    let config: SampleConfig = figment
        .extract()
        .context("extract sample config from figment")?;
    ensure!(
        !config.value,
        "expected valid candidate to override invalid env file"
    );
    ensure!(
        std::fs::metadata(&invalid).is_ok(),
        "expected invalid file retained for later diagnostics"
    );
    Ok(())
}

#[rstest]
fn load_first_with_errors_reports_preceding_failures(
    env_guards: EnvScope,
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let _guards = env_guards;
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let missing = dir_path.join("absent.toml");
    let valid = dir_path.join("valid.toml");
    std::fs::write(&valid, "value = true").context("write valid config")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .add_explicit_path(valid.clone())
        .build();

    let (loaded_fig, errors) = discovery.load_first_with_errors();

    ensure!(
        loaded_fig.is_some(),
        "expected successful load from valid fallback"
    );
    ensure!(
        errors.iter().any(|err| match err.as_ref() {
            OrthoError::File { path, .. } => path == &missing,
            _ => false,
        }),
        "expected discovery error collection to capture missing required candidate",
    );
    Ok(())
}

#[rstest]
fn partitioned_errors_surface_required_failures(
    env_guards: EnvScope,
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let _guards = env_guards;
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let missing = dir_path.join("absent.toml");
    let valid = dir_path.join("valid.toml");
    std::fs::write(&valid, "value = true").context("write valid config")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .add_explicit_path(valid.clone())
        .build();

    let outcome = discovery.load_first_partitioned();

    ensure!(outcome.figment.is_some(), "expected fallback figment");
    ensure!(
        outcome
            .required_errors
            .iter()
            .any(|err| match err.as_ref() {
                OrthoError::File { path, .. } => path == &missing,
                _ => false,
            }),
        "expected missing required candidate to be retained",
    );
    ensure!(
        outcome.optional_errors.is_empty(),
        "expected optional errors to remain empty when only required path fails",
    );
    Ok(())
}

#[rstest]
fn required_paths_emit_missing_errors(
    env_guards: EnvScope,
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let _guards = env_guards;
    let temp_dir = config_temp_dir?;
    let missing = temp_dir.path().join("absent.toml");

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .build();
    let (_, errors) = discovery.load_first_with_errors();

    ensure!(
        errors.iter().any(|err| match err.as_ref() {
            OrthoError::File { path, .. } => path == &missing,
            _ => false,
        }),
        "expected missing required path error"
    );
    Ok(())
}
