//! Tests for struct-level discovery attributes.
//!
//! Validates that the `#[ortho_config(discovery(...))]` attribute correctly
//! customises configuration discovery, including CLI flag names, environment
//! variable names, and file paths. Tests cover loading via CLI flags,
//! environment variables, `XDG_CONFIG_HOME`, dotfile fallback, defaults, and
//! error handling for missing or malformed configurations.
//!
//! Every case injects a [`MapEnv`] that also pins the working directory, so no
//! test changes the process environment or its working directory and the cases
//! run concurrently.

use anyhow::{Context, Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig, OrthoError, OrthoResult};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP_",
    discovery(
        app_name = "demo_app",
        config_file_name = "demo.toml",
        dotfile_name = ".demo.toml",
        project_file_name = ".demo.toml",
        config_cli_long = "config",
        config_cli_short = 'c',
        config_cli_visible = true
    )
)]
struct DiscoveryConfig {
    #[ortho_config(default = 1)]
    value: u32,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "POLICY_",
    discovery(
        app_name = "policy_app",
        config_file_name = "policy.toml",
        project_file_name = "project.toml",
        config_cli_long = "config",
        config_cli_visible = true,
        automatic_mode = "stack_scopes",
        scope_order = ["user", "project"]
    )
)]
struct PolicyDiscoveryConfig {
    #[ortho_config(default = 1)]
    value: u32,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "OPTIONAL_POLICY_",
    discovery(
        app_name = "optional_policy_app",
        config_cli_long = "config",
        config_cli_visible = true,
        explicit_mode = "optional"
    )
)]
struct OptionalPolicyDiscoveryConfig {
    #[ortho_config(default = 1)]
    value: u32,
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory for {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("write config file {}", path.display()))?;
    Ok(())
}

/// A closed environment rooted at `dir`: no host variable, home, or XDG
/// directory can contribute, and the working directory is a subdirectory of
/// `dir` that holds nothing unless a test puts something there.
fn isolated_env(dir: &Path) -> MapEnv {
    MapEnv::new()
        .with_current_dir(dir.join("cwd-empty"))
        .with_var("XDG_CONFIG_HOME", dir.join("xdg-empty"))
        .with_var("XDG_CONFIG_DIRS", dir.join("xdg-dirs-empty"))
}

/// Load `T` from `args` through the injected `env`.
fn load<T: OrthoConfig>(args: &[&str], env: MapEnv) -> OrthoResult<T> {
    T::load_from_iter_with_env(args.iter().copied(), Arc::new(env))
}

fn create_test_config(dir: &Path, filename: &str, value: u32) -> Result<std::path::PathBuf> {
    let path = dir.join(filename);
    write_file(&path, &format!("value = {value}"))?;
    Ok(path)
}

fn validation_error(message: impl Into<String>) -> OrthoResult<()> {
    Err(OrthoError::Validation {
        key: "discovery_attributes".to_owned(),
        message: message.into(),
    }
    .into())
}

fn utf8(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow!("temporary path must be valid UTF-8"))
}

struct CliFlagCase {
    flag: &'static str,
    filename: &'static str,
    file_contents: &'static str,
    expected_value: Option<u32>,
    description: &'static str,
}

#[rstest]
#[case(CliFlagCase {
    flag: "--config",
    filename: "explicit.toml",
    file_contents: "value = 41",
    expected_value: Some(41),
    description: "load config from long flag",
})]
#[case(CliFlagCase {
    flag: "--config",
    filename: "missing.toml",
    file_contents: "",
    expected_value: None,
    description: "error when CLI path missing",
})]
#[case(CliFlagCase {
    flag: "-c",
    filename: "short.toml",
    file_contents: "value = 17",
    expected_value: Some(17),
    description: "load config from short flag",
})]
fn cli_flag_config_loading(#[case] case: CliFlagCase) -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let config_path = if case.file_contents.is_empty() {
        dir.path().join(case.filename)
    } else if let Some(value) = case.expected_value {
        let expected_contents = format!("value = {value}");
        ensure!(
            case.file_contents == expected_contents,
            "case {} must provide canonical contents",
            case.description
        );
        create_test_config(dir.path(), case.filename, value)?
    } else {
        let path = dir.path().join(case.filename);
        write_file(&path, case.file_contents)?;
        path
    };

    let args = ["prog", case.flag, utf8(&config_path)?];
    let env = isolated_env(dir.path());

    if let Some(value) = case.expected_value {
        let cfg = load::<DiscoveryConfig>(&args, env).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.value == value,
            "{}: expected {value}, got {}",
            case.description,
            cfg.value
        );
    } else {
        match load::<DiscoveryConfig>(&args, env) {
            Ok(_) => ensure!(false, "{}: expected Err but got Ok", case.description),
            Err(err) => ensure!(
                matches!(&*err, OrthoError::File { .. }),
                "{}: unexpected error {err:?}",
                case.description
            ),
        }
    }
    Ok(())
}

#[test]
fn env_var_overrides_default_locations() -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let config_path = create_test_config(dir.path(), "env.toml", 99)?;
    let env = isolated_env(dir.path()).with_var("APP_CONFIG_PATH", &config_path);

    let cfg = load::<DiscoveryConfig>(&["prog"], env).map_err(|err| anyhow!(err))?;
    ensure!(cfg.value == 99, "expected 99, got {}", cfg.value);
    Ok(())
}

fn load_xdg_config<F>(setup: F) -> OrthoResult<DiscoveryConfig>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let result = (|| -> Result<DiscoveryConfig> {
        let dir = TempDir::new().context("create temp dir")?;
        let xdg_home = dir.path().join("xdg");
        let app_dir = xdg_home.join("demo_app");
        fs::create_dir_all(&app_dir).context("create XDG application directory")?;
        setup(&app_dir)?;
        let env = isolated_env(dir.path()).with_var("XDG_CONFIG_HOME", &xdg_home);
        load::<DiscoveryConfig>(&["prog"], env).map_err(|err| anyhow!(err))
    })();
    result.map_err(|err| {
        OrthoError::Validation {
            key: "discovery_attributes".to_owned(),
            message: err.to_string(),
        }
        .into()
    })
}

fn assert_xdg_cfg_value<F>(expected: u32, setup: F) -> OrthoResult<()>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let cfg = load_xdg_config(setup)?;
    if cfg.value != expected {
        return validation_error(format!("expected value {expected}, got {}", cfg.value));
    }
    Ok(())
}

#[test]
fn xdg_config_home_missing_uses_default() -> OrthoResult<()> {
    assert_xdg_cfg_value(1, |_| Ok(()))
}

#[test]
fn xdg_config_home_reads_custom_file() -> OrthoResult<()> {
    assert_xdg_cfg_value(64, |app_dir| {
        write_file(&app_dir.join("demo.toml"), "value = 64")?;
        Ok(())
    })
}

#[test]
fn dotfile_fallback_uses_custom_name() -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let _ = create_test_config(dir.path(), ".demo.toml", 23)?;
    let env = isolated_env(dir.path()).with_current_dir(dir.path());

    let cfg = load::<DiscoveryConfig>(&["prog"], env).map_err(|err| anyhow!(err))?;
    ensure!(cfg.value == 23, "expected 23, got {}", cfg.value);
    Ok(())
}

#[test]
fn defaults_apply_when_no_config_found() -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let cfg =
        load::<DiscoveryConfig>(&["prog"], isolated_env(dir.path())).map_err(|err| anyhow!(err))?;
    ensure!(cfg.value == 1, "expected default 1, got {}", cfg.value);
    Ok(())
}

#[test]
fn policy_attribute_stacks_scopes_and_suppresses_them_after_selection() -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let user_home = dir.path().join("user");
    let project_root = dir.path().join("project");
    let selected = dir.path().join("selected.toml");
    let _ = create_test_config(&user_home.join("policy_app"), "policy.toml", 20)?;
    let _ = create_test_config(&project_root, "project.toml", 30)?;
    let _ = create_test_config(dir.path(), "selected.toml", 40)?;
    let env = isolated_env(dir.path())
        .with_var("XDG_CONFIG_HOME", &user_home)
        .with_current_dir(&project_root);

    let stacked =
        load::<PolicyDiscoveryConfig>(&["prog"], env.clone()).map_err(|err| anyhow!(err))?;
    ensure!(
        stacked.value == 30,
        "project scope must override user scope"
    );

    let explicit = load::<PolicyDiscoveryConfig>(&["prog", "--config", utf8(&selected)?], env)
        .map_err(|err| anyhow!(err))?;
    ensure!(
        explicit.value == 40,
        "--config must suppress automatic scopes"
    );
    Ok(())
}

#[test]
fn optional_policy_selector_ignores_a_missing_config_path() -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let missing = dir.path().join("missing.toml");
    let config = load::<OptionalPolicyDiscoveryConfig>(
        &["prog", "--config", utf8(&missing)?],
        isolated_env(dir.path()),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        config.value == 1,
        "optional missing selection must use defaults"
    );
    Ok(())
}

#[test]
fn error_on_malformed_config() -> Result<()> {
    let dir = TempDir::new().context("create temp dir")?;
    let config_path = dir.path().join("broken.toml");
    write_file(&config_path, "value = ???")?;
    let env = isolated_env(dir.path()).with_var("APP_CONFIG_PATH", &config_path);

    let err = match load::<DiscoveryConfig>(&["prog"], env) {
        Ok(cfg) => return Err(anyhow!("expected parse failure, got config {cfg:?}")),
        Err(err) => err,
    };

    ensure!(
        matches!(&*err, OrthoError::File { .. }),
        "unexpected error: {err:?}"
    );
    Ok(())
}
