//! Injected sources for the profile scenarios (`profiles.feature`).
//!
//! The scenarios supply their configuration file and environment through a
//! temporary directory and a closed-set [`MapEnv`], so no scenario writes the
//! process environment or changes the working directory.

use ortho_config::{MapEnv, OrthoError, OrthoResult};
use serde_json::Value;
use std::sync::Arc;
use tempfile::TempDir;

/// Environment keys and values the scenario sets, already prefixed `APP_`.
pub struct ScenarioEnvironment {
    /// Value of the `APP_PROFILE` selector, when the scenario names one.
    pub selector: Option<String>,
    /// Lower-case configuration keys paired with their environment values.
    pub keys: Vec<(String, String)>,
}

/// A prepared scenario: the injected source and the directory that backs it.
pub struct ScenarioSources {
    /// Backs the configuration file; it must outlive every load.
    _directory: TempDir,
    /// One closed-set map serving both the discovery and merge layers.
    pub environment: Arc<MapEnv>,
}

fn setup_error(message: String) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: "profile-scenario".to_owned(),
        message,
    })
}

/// Stage `file_value` (when present) and the scenario environment.
///
/// One closed-set map backs both layers, so neither the host's environment nor
/// its configuration directories can reach the scenario. The file is selected
/// with an absolute `APP_CONFIG_PATH`, so no working directory is involved.
pub fn prepare(
    scenario: &ScenarioEnvironment,
    file_value: Option<&Value>,
) -> OrthoResult<ScenarioSources> {
    let directory = tempfile::tempdir()
        .map_err(|err| setup_error(format!("profile scenario directory failed: {err}")))?;
    let mut environment = MapEnv::new();
    if let Some(value) = scenario.selector.as_ref() {
        environment = environment.with_var("APP_PROFILE", value);
    }
    for (key, value) in &scenario.keys {
        environment = environment.with_var(format!("APP_{}", key.to_ascii_uppercase()), value);
    }
    if let Some(value) = file_value {
        let content = ortho_config::toml::to_string(value)
            .map_err(|err| setup_error(format!("profile scenario file failed: {err}")))?;
        cap_std::fs::Dir::open_ambient_dir(directory.path(), cap_std::ambient_authority())
            .and_then(|dir| dir.write(".app.toml", content))
            .map_err(|err| setup_error(format!("profile scenario file failed: {err}")))?;
        environment = environment.with_var("APP_CONFIG_PATH", directory.path().join(".app.toml"));
    }
    // The working directory and the XDG bases name nonexistent subdirectories
    // of the fixture, so neither the process's directory nor the host's
    // `/etc/xdg` can supply a file; the scenario file is reached only through
    // `APP_CONFIG_PATH`.
    let environment = environment
        .with_current_dir(directory.path().join("empty-cwd"))
        .with_var("XDG_CONFIG_HOME", directory.path().join("empty-xdg-home"))
        .with_var("XDG_CONFIG_DIRS", directory.path().join("empty-xdg-dirs"));
    Ok(ScenarioSources {
        _directory: directory,
        environment: Arc::new(environment),
    })
}
