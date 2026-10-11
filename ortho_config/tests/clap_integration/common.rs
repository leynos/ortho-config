//! Shared types and helpers for the CLI integration tests.

use anyhow::{Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoResult};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, fmt, path::Path, sync::Arc};

pub(crate) use ortho_config::{OrthoConfig, OrthoError};

#[path = "../clap_test_utils.rs"]
mod clap_test_utils;
use clap_test_utils::ConfigValueAssertions;
pub(crate) use clap_test_utils::assert_config_values;

#[path = "../support/isolated_env.rs"]
mod isolated_env;
use isolated_env::with_host_overrides;

#[path = "../support/to_anyhow.rs"]
mod to_anyhow;
pub(crate) use to_anyhow::ToAnyhow;

pub(crate) const DEFAULT_RECIPIENT: &str = "World";
pub(crate) const DEFAULT_SALUTATIONS: &[&str] = &["Hello"];

fn default_recipient() -> String {
    String::from(DEFAULT_RECIPIENT)
}

fn default_salutations() -> Vec<String> {
    DEFAULT_SALUTATIONS
        .iter()
        .map(|s| String::from(*s))
        .collect()
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Clone)]
pub(crate) struct TestConfig {
    #[ortho_config(default = default_recipient())]
    pub(crate) recipient: String,
    #[serde(default = "default_salutations")]
    #[ortho_config(default = default_salutations())]
    pub(crate) salutations: Vec<String>,
    #[serde(default)]
    #[ortho_config(default = false)]
    pub(crate) is_excited: bool,
    #[serde(default)]
    #[ortho_config(default = false)]
    pub(crate) is_quiet: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) other: Option<String>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            recipient: default_recipient(),
            salutations: default_salutations(),
            is_excited: false,
            is_quiet: false,
            sample_value: None,
            other: None,
        }
    }
}

impl ConfigValueAssertions for TestConfig {
    fn assert_values(
        &self,
        expected_sample: Option<&'static str>,
        expected_other: Option<&'static str>,
    ) -> Result<()> {
        let expected = ExpectedConfig {
            sample_value: expected_sample,
            other: expected_other,
            ..ExpectedConfig::default()
        };
        assert_config_eq(self, &expected).to_anyhow()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExpectedConfig {
    pub recipient: &'static str,
    pub salutations: &'static [&'static str],
    pub is_excited: bool,
    pub is_quiet: bool,
    pub sample_value: Option<&'static str>,
    pub other: Option<&'static str>,
}

impl Default for ExpectedConfig {
    fn default() -> Self {
        Self {
            recipient: DEFAULT_RECIPIENT,
            salutations: DEFAULT_SALUTATIONS,
            is_excited: false,
            is_quiet: false,
            sample_value: None,
            other: None,
        }
    }
}

/// Shared harness for CLI cases: writes the fixture files, builds the injected
/// environment, rewrites path-taking CLI arguments, loads the configuration
/// through both injected sources, then runs the caller's validation closure.
pub(crate) fn run_config_case<T, F>(
    files: &[(&str, &str)],
    env: &[(&str, &str)],
    cli_args: &[&str],
    validate: F,
) -> Result<T>
where
    T: OrthoConfig,
    F: FnOnce(&T) -> Result<()>,
{
    let temp_dir = tempfile::tempdir()?;
    write_fixtures(temp_dir.path(), files)?;
    let env_map = build_env(temp_dir.path(), files, env);
    let args = resolve_args(temp_dir.path(), cli_args);
    let source = Arc::new(env_map);
    let config = T::load_from_iter_with_sources(args, source.clone(), source).to_anyhow()?;
    validate(&config)?;
    Ok(config)
}

/// Write each fixture beneath `root`, failing on the first that cannot be
/// created.
fn write_fixtures(root: &Path, files: &[(&str, &str)]) -> Result<()> {
    let cap = Dir::open_ambient_dir(root, ambient_authority())?;
    for (path, contents) in files {
        cap.write(path, contents.as_bytes())?;
    }
    Ok(())
}

/// Build the injected environment for one case.
///
/// A `CONFIG_PATH` entry names a fixture file, so its value is resolved
/// against `root`; every other variable is passed through untouched. When the
/// fixtures include the default dotfile and no selector was supplied, the
/// selector is pointed at that file so the case does not depend on the
/// process working directory. [`with_host_overrides`] then pins the XDG bases,
/// closing the platform default that would otherwise reach `/etc/xdg`.
fn build_env(root: &Path, files: &[(&str, &str)], env: &[(&str, &str)]) -> MapEnv {
    let mut env_map = MapEnv::new();
    for (key, value) in env {
        if *key == "CONFIG_PATH" {
            env_map.insert(*key, fixture_path(root, value));
        } else {
            env_map.insert(*key, value);
        }
    }
    if files.iter().any(|(name, _)| *name == ".config.toml")
        && !env.iter().any(|(name, _)| *name == "CONFIG_PATH")
    {
        env_map.insert("CONFIG_PATH", root.join(".config.toml"));
    }
    with_host_overrides(env_map, root)
}

/// Resolve a case's relative path against the temporary fixture root.
///
/// Absolute values are preserved so a case can deliberately point outside the
/// fixture.
fn fixture_path(root: &Path, value: &str) -> OsString {
    if Path::new(value).is_absolute() {
        OsString::from(value)
    } else {
        root.join(value).into_os_string()
    }
}

/// Rewrite CLI arguments so a path-taking flag names a fixture file.
fn resolve_args(root: &Path, cli_args: &[&str]) -> Vec<OsString> {
    let mut args = Vec::with_capacity(cli_args.len());
    let mut next_is_path = false;
    for arg in cli_args {
        let value = if next_is_path {
            fixture_path(root, arg)
        } else {
            OsString::from(*arg)
        };
        next_is_path = matches!(value.to_str(), Some("--config-path" | "--config"));
        args.push(value);
    }
    args
}

pub(crate) fn assert_ortho_error<T, F>(
    result: OrthoResult<T>,
    expected_variant: &str,
    predicate: F,
) -> Result<()>
where
    T: fmt::Debug,
    F: FnOnce(&OrthoError) -> bool,
{
    match result {
        Ok(value) => Err(anyhow!(
            "expected {expected_variant} error, got success: {value:?}"
        )),
        Err(err) => {
            ensure!(
                predicate(err.as_ref()),
                "expected {expected_variant} error, got {err:?}"
            );
            Ok(())
        }
    }
}

fn validation_mismatch<T>(key: &str, expected: &str, actual: T) -> OrthoResult<()>
where
    T: fmt::Debug,
{
    Err(OrthoError::Validation {
        key: key.to_owned(),
        message: format!("expected {expected}, got {actual:?}"),
    }
    .into())
}

pub(crate) fn assert_config_eq(config: &TestConfig, expected: &ExpectedConfig) -> OrthoResult<()> {
    if config.recipient != expected.recipient {
        return validation_mismatch("recipient", expected.recipient, &config.recipient);
    }

    let actual_salutations: Vec<&str> = config.salutations.iter().map(String::as_str).collect();
    if actual_salutations.as_slice() != expected.salutations {
        return validation_mismatch(
            "salutations",
            &format!("{:?}", expected.salutations),
            actual_salutations,
        );
    }

    if config.is_excited != expected.is_excited {
        return validation_mismatch(
            "is_excited",
            &expected.is_excited.to_string(),
            config.is_excited,
        );
    }

    if config.is_quiet != expected.is_quiet {
        return validation_mismatch("is_quiet", &expected.is_quiet.to_string(), config.is_quiet);
    }

    if config.sample_value.as_deref() != expected.sample_value {
        return validation_mismatch(
            "sample_value",
            &format!("{:?}", expected.sample_value),
            config.sample_value.as_deref(),
        );
    }

    if config.other.as_deref() != expected.other {
        return validation_mismatch(
            "other",
            &format!("{:?}", expected.other),
            config.other.as_deref(),
        );
    }

    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Clone)]
pub(crate) struct OptionConfig {
    pub(crate) maybe: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Clone)]
pub(crate) struct RequiredConfig {
    pub(crate) sample_value: String,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Clone)]
pub(crate) struct ConflictConfig {
    pub(crate) second: Option<String>,
    pub(crate) sample: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Clone)]
pub(crate) struct RenamedPathConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample: Option<String>,
    #[serde(skip)]
    #[ortho_config(cli_long = "config")]
    pub(crate) config_path: Option<std::path::PathBuf>,
}
