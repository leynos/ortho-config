//! Loading behaviour tests for discovery.

use std::io::Write as _;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir as Utf8Dir};
use rstest::rstest;
use serde::Deserialize;
use tempfile::TempDir;

use super::super::*;
use super::fixtures::{config_temp_dir, sample_config_file};
use crate::MapEnv;

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

/// Returns true when `error` names a file called `file_name`.
///
/// A single discovery failure is usually returned directly; once more than one
/// diagnostic survives it wraps them in [`OrthoError::Aggregate`], so the helper
/// searches both shapes. Comparing the file name rather than the full path
/// keeps the assertion portable: Windows canonical paths carry a `\\?\` prefix
/// under `std::fs` but not under the loader's `dunce` canonicalisation.
fn names_file_error(error: &OrthoError, file_name: &str) -> bool {
    match error {
        OrthoError::File { path, .. } => {
            path.file_name().and_then(|name| name.to_str()) == Some(file_name)
        }
        OrthoError::Aggregate(aggregated) => aggregated
            .iter()
            .any(|inner| names_file_error(inner, file_name)),
        _ => false,
    }
}

/// Assert that `result` failed with a file error naming `file_name`.
fn expect_file_error(
    result: crate::OrthoResult<Option<figment::Figment>>,
    file_name: &str,
) -> Result<()> {
    match result {
        Ok(_) => Err(anyhow!("expected discovery to fail for {file_name}")),
        Err(error) => {
            ensure!(
                names_file_error(error.as_ref(), file_name),
                "expected a file error for {file_name}, got {error:?}"
            );
            Ok(())
        }
    }
}

#[rstest]
fn load_first_reads_first_existing_file(
    sample_config_file: Result<(TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp_dir, _config_path) = sample_config_file?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(
            MapEnv::new().with_var("XDG_CONFIG_HOME", temp_dir.path()),
        ))
        .build();
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
fn load_first_fails_when_selector_candidate_is_invalid(
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let invalid = dir_path.join("broken.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "broken.toml", "is_enabled = ???")?;
    write_cap_file(&dir, "valid.toml", "is_enabled = false")?;
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(
            MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &invalid),
        ))
        .add_explicit_path(valid)
        .build();

    // Optional explicit paths are generated before the selector, so this valid
    // path is the walk's first success. The required selector must still be
    // checked; otherwise that success would hide the selector's error.
    let outcome = discovery.load_first_partitioned();
    ensure!(
        outcome.figment.is_some(),
        "expected the valid explicit fallback to load"
    );
    ensure!(
        !outcome.required_errors.is_empty(),
        "expected the invalid selector to be retained as a required error"
    );
    expect_file_error(discovery.load_first(), "broken.toml")?;
    ensure!(
        dir.metadata("broken.toml").is_ok(),
        "expected invalid file retained for later diagnostics"
    );
    Ok(())
}

#[rstest]
fn load_first_fails_when_required_path_is_missing(config_temp_dir: Result<TempDir>) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let missing = dir_path.join("absent.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "valid.toml", "is_enabled = true")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
        .add_required_path(&missing)
        .add_explicit_path(valid)
        .build();

    let outcome = discovery.load_first_partitioned();
    ensure!(
        outcome.figment.is_some(),
        "expected the optional fallback to load"
    );
    expect_file_error(discovery.load_first(), "absent.toml")
}

#[rstest]
fn load_first_fails_when_selector_path_is_missing(config_temp_dir: Result<TempDir>) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let missing = dir_path.join("absent.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "valid.toml", "is_enabled = true")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(
            MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &missing),
        ))
        .add_explicit_path(valid)
        .build();

    let outcome = discovery.load_first_partitioned();
    ensure!(
        outcome.figment.is_some(),
        "expected the optional fallback to load"
    );
    expect_file_error(discovery.load_first(), "absent.toml")
}

#[rstest]
fn load_first_fails_when_selector_extends_chain_is_broken(
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let child = dir_path.join("child.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(
        &dir,
        "child.toml",
        "extends = \"missing-base.toml\"\nis_enabled = true",
    )?;
    write_cap_file(&dir, "valid.toml", "is_enabled = false")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(
            MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &child),
        ))
        .add_explicit_path(valid)
        .build();

    expect_file_error(discovery.load_first(), "missing-base.toml")
}

/// A selector that exists but cannot be read is still a required failure.
#[cfg(unix)]
#[rstest]
fn load_first_fails_when_selector_is_unreadable(config_temp_dir: Result<TempDir>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let unreadable = dir_path.join("unreadable.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "unreadable.toml", "is_enabled = true")?;
    write_cap_file(&dir, "valid.toml", "is_enabled = false")?;
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))?;
    // A privileged runner can read the file regardless of its mode, so skip
    // rather than assert something the environment cannot exercise.
    if std::fs::read(&unreadable).is_ok() {
        return Ok(());
    }

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(
            MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &unreadable),
        ))
        .add_explicit_path(valid)
        .build();

    expect_file_error(discovery.load_first(), "unreadable.toml")
}

/// A selector naming a file in a format disabled at build time fails closed.
#[cfg(not(feature = "json5"))]
#[rstest]
fn load_first_fails_when_selector_format_is_disabled(
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let selected = dir_path.join("selected.json5");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "selected.json5", "{ is_enabled: true }")?;
    write_cap_file(&dir, "valid.toml", "is_enabled = false")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(
            MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &selected),
        ))
        .add_explicit_path(valid)
        .build();

    expect_file_error(discovery.load_first(), "selected.json5")
}

/// An unset or empty selector selects nothing, so automatic discovery loads.
#[rstest]
#[case::unset(None)]
#[case::empty(Some(""))]
fn load_first_ignores_unset_or_empty_selector(
    #[case] selector_value: Option<&str>,
    sample_config_file: Result<(TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp_dir, _config_path) = sample_config_file?;
    let mut env = MapEnv::new().with_var("XDG_CONFIG_HOME", temp_dir.path());
    if let Some(value) = selector_value {
        env = env.with_var("HELLO_WORLD_CONFIG_PATH", value);
    }

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .env_source(Arc::new(env))
        .build();
    let figment = discovery
        .load_first()
        .map_err(|err| anyhow!(err))?
        .ok_or_else(|| anyhow!("expected the automatic candidate to load"))?;
    let config: SampleConfig = figment
        .extract()
        .context("extract sample config from figment")?;
    ensure!(
        config.is_enabled,
        "expected the automatic candidate's value, got {config:?}"
    );
    Ok(())
}

#[rstest]
fn load_first_with_errors_reports_preceding_failures(
    config_temp_dir: Result<TempDir>,
) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let missing = dir_path.join("absent.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "valid.toml", "is_enabled = true")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
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
fn partitioned_errors_surface_required_failures(config_temp_dir: Result<TempDir>) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let dir_path = temp_dir.path();
    let missing = dir_path.join("absent.toml");
    let valid = dir_path.join("valid.toml");
    let dir = open_cap_utf8_dir(dir_path)?;
    write_cap_file(&dir, "valid.toml", "is_enabled = true")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
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
fn required_paths_emit_missing_errors(config_temp_dir: Result<TempDir>) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let missing = temp_dir.path().join("absent.toml");

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
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

#[path = "loading_required.rs"]
mod loading_required;
