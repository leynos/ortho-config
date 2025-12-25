//! Tests for struct-level discovery attributes.
//!
//! Validates that the `#[ortho_config(discovery(...))]` attribute correctly
//! customises configuration discovery, including CLI flag names, environment
//! variable names, and file paths. Tests cover loading via CLI flags,
//! environment variables, `XDG_CONFIG_HOME`, dotfile fallback, defaults, and
//! error handling for missing or malformed configurations.

use anyhow::{Context, Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoError, OrthoResult};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;
use test_helpers::env as test_env;

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

#[fixture]
fn env_lock() -> test_env::EnvVarLock {
    test_env::lock()
}

fn write_file(path: &std::path::Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory for {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("write config file {}", path.display()))?;
    Ok(())
}

fn clear_support_env() -> Vec<test_env::EnvVarGuard> {
    ["XDG_CONFIG_HOME", "HOME", "USERPROFILE"]
        .into_iter()
        .map(test_env::remove_var)
        .collect()
}

fn setup_clean_env() -> Vec<test_env::EnvVarGuard> {
    let mut guards = clear_support_env();
    guards.push(test_env::remove_var("APP_CONFIG_PATH"));
    guards
}

fn create_test_config(
    dir: &std::path::Path,
    filename: &str,
    value: u32,
) -> Result<std::path::PathBuf> {
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

static CWD_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

struct CwdGuard {
    original: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CwdGuard {
    fn new() -> Result<Self> {
        #[expect(
            clippy::expect_used,
            reason = "Tests must fail fast if the CWD mutex is poisoned"
        )]
        let lock = CWD_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("CwdGuard mutex poisoned while acquiring lock");
        let original = env::current_dir().context("capture current directory")?;
        Ok(Self {
            original,
            _lock: lock,
        })
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        #[expect(
            clippy::expect_used,
            reason = "Restoring the original CWD must not fail in tests"
        )]
        env::set_current_dir(&self.original)
            .expect("restore original working directory in CwdGuard::drop");
    }
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
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn cli_flag_config_loading(
    #[case] case: CliFlagCase,
    _env_lock: test_env::EnvVarLock,
) -> Result<()> {
    let _env = setup_clean_env();
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

    let args = [
        "prog",
        case.flag,
        config_path
            .to_str()
            .ok_or_else(|| anyhow!("temporary path must be valid UTF-8"))?,
    ];

    if let Some(value) = case.expected_value {
        let cfg = DiscoveryConfig::load_from_iter(args).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.value == value,
            "{}: expected {value}, got {}",
            case.description,
            cfg.value
        );
    } else {
        match DiscoveryConfig::load_from_iter(args) {
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

#[rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn env_var_overrides_default_locations(_env_lock: test_env::EnvVarLock) -> Result<()> {
    let _env = setup_clean_env();
    let dir = TempDir::new().context("create temp dir")?;
    let config_path = create_test_config(dir.path(), "env.toml", 99)?;

    let guard = test_env::set_var("APP_CONFIG_PATH", &config_path);
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
    drop(guard);

    ensure!(cfg.value == 99, "expected 99, got {}", cfg.value);
    Ok(())
}

fn load_xdg_config<F>(setup: F) -> OrthoResult<DiscoveryConfig>
where
    F: FnOnce(&std::path::Path) -> Result<()>,
{
    let result = (|| -> Result<DiscoveryConfig> {
        let _env = setup_clean_env();
        let dir = TempDir::new().context("create temp dir")?;
        let xdg_home = dir.path().join("xdg");
        let app_dir = xdg_home.join("demo_app");
        fs::create_dir_all(&app_dir).context("create XDG application directory")?;
        setup(&app_dir)?;
        let xdg_value = xdg_home
            .to_str()
            .ok_or_else(|| anyhow!("XDG path must be UTF-8"))?
            .to_owned();
        let guard = test_env::set_var("XDG_CONFIG_HOME", &xdg_value);
        let cfg = DiscoveryConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        drop(guard);
        Ok(cfg)
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
    F: FnOnce(&std::path::Path) -> Result<()>,
{
    let cfg = load_xdg_config(setup)?;
    if cfg.value != expected {
        return validation_error(format!("expected value {expected}, got {}", cfg.value));
    }
    Ok(())
}

#[rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn xdg_config_home_missing_uses_default(_env_lock: test_env::EnvVarLock) -> OrthoResult<()> {
    assert_xdg_cfg_value(1, |_| Ok(()))
}

#[rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn xdg_config_home_reads_custom_file(_env_lock: test_env::EnvVarLock) -> OrthoResult<()> {
    assert_xdg_cfg_value(64, |app_dir| {
        write_file(&app_dir.join("demo.toml"), "value = 64")?;
        Ok(())
    })
}

#[rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn dotfile_fallback_uses_custom_name(_env_lock: test_env::EnvVarLock) -> Result<()> {
    let _env = setup_clean_env();
    let dir = TempDir::new().context("create temp dir")?;
    let _ = create_test_config(dir.path(), ".demo.toml", 23)?;

    let _cwd_guard = CwdGuard::new()?;
    env::set_current_dir(dir.path()).context("set current dir")?;
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;

    ensure!(cfg.value == 23, "expected 23, got {}", cfg.value);
    Ok(())
}

#[rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn defaults_apply_when_no_config_found(_env_lock: test_env::EnvVarLock) -> Result<()> {
    let _env = setup_clean_env();
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
    ensure!(cfg.value == 1, "expected default 1, got {}", cfg.value);
    Ok(())
}

#[rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "Env lock fixture is intentionally unused; it holds the lock for the test."
)]
fn error_on_malformed_config(_env_lock: test_env::EnvVarLock) -> Result<()> {
    let _env = setup_clean_env();
    let dir = TempDir::new().context("create temp dir")?;
    let config_path = dir.path().join("broken.toml");
    write_file(&config_path, "value = ???")?;

    let guard = test_env::set_var("APP_CONFIG_PATH", &config_path);
    let err = match DiscoveryConfig::load_from_iter(["prog"]) {
        Ok(cfg) => return Err(anyhow!("expected parse failure, got config {cfg:?}")),
        Err(err) => err,
    };
    drop(guard);

    ensure!(
        matches!(&*err, OrthoError::File { .. }),
        "unexpected error: {err:?}"
    );
    Ok(())
}
