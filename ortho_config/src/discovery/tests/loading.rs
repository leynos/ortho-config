//! Loading behaviour tests for discovery.

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir as Utf8Dir};
use rstest::rstest;
use serde::Deserialize;
use tempfile::TempDir;
use test_helpers::env::{self as test_env, EnvScope};

use super::super::*;
use super::fixtures::{config_temp_dir, env_guards, sample_config_file};

#[derive(Debug, Deserialize)]
struct SampleConfig {
    is_enabled: bool,
}

fn open_cap_utf8_dir(path: &std::path::Path) -> Result<Utf8Dir> {
    let utf8_path = Utf8Path::from_path(path)
        .ok_or_else(|| anyhow!("temporary directory path is not valid UTF-8: {path:?}"))?;
    Utf8Dir::open_ambient_dir(utf8_path, ambient_authority())
        .context("open temporary directory with cap-std")
}

fn write_cap_file(dir: &Utf8Dir, name: &str, contents: &str) -> Result<()> {
    let mut file = dir.create(name).with_context(|| format!("create {name}"))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("write {name}"))?;
    Ok(())
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
    ensure!(
        config.is_enabled,
        "expected loaded config to set is_enabled=true"
    );
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
    let dir = open_cap_utf8_dir(dir_path)?;
    {
        let mut invalid_file = dir.create("broken.toml").context("create invalid config")?;
        invalid_file
            .write_all(b"is_enabled = ???")
            .context("write invalid config")?;
    }
    {
        let mut valid_file = dir.create("valid.toml").context("create valid config")?;
        valid_file
            .write_all(b"is_enabled = false")
            .context("write valid config")?;
    }
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
        !config.is_enabled,
        "expected valid candidate to override invalid env file"
    );
    ensure!(
        dir.metadata("broken.toml").is_ok(),
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
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "valid.toml", "is_enabled = true")?;

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
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "valid.toml", "is_enabled = true")?;

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
