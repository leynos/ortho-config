//! Tests for struct-level discovery attributes.
//!
//! Validates that the `#[ortho_config(discovery(...))]` attribute correctly
//! customises configuration discovery, including CLI flag names, environment
//! variable names, and file paths. Tests cover loading via CLI flags,
//! environment variables, `XDG_CONFIG_HOME`, dotfile fallback, defaults, and
//! error handling for missing or malformed configurations.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
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

fn write_file(path: &std::path::Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("parent dir")).expect("create parent directory");
    fs::write(path, contents).expect("write config file");
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

fn create_test_config(dir: &std::path::Path, filename: &str, value: u32) -> std::path::PathBuf {
    let path = dir.join(filename);
    write_file(&path, &format!("value = {value}"));
    path
}

struct CwdGuard {
    original: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.original).expect("restore current dir");
    }
}

#[rstest]
#[case(
    "--config",
    "explicit.toml",
    "value = 41",
    Some(41),
    "load config from long flag"
)]
#[case("--config", "missing.toml", "", None, "error when CLI path missing")]
#[case(
    "-c",
    "short.toml",
    "value = 17",
    Some(17),
    "load config from short flag"
)]
fn cli_flag_config_loading(
    #[case] flag: &str,
    #[case] filename: &str,
    #[case] file_contents: &str,
    #[case] expected_value: Option<u32>,
    #[case] description: &str,
) {
    let _env = setup_clean_env();
    let dir = TempDir::new().expect("temp dir");
    let config_path = if file_contents.is_empty() {
        dir.path().join(filename)
    } else if let Some(value) = expected_value {
        let expected_contents = format!("value = {value}");
        assert_eq!(
            file_contents, expected_contents,
            "case {description} must provide canonical contents"
        );
        create_test_config(dir.path(), filename, value)
    } else {
        let path = dir.path().join(filename);
        write_file(&path, file_contents);
        path
    };

    let args = [
        "prog",
        flag,
        config_path
            .to_str()
            .expect("temporary paths must be valid UTF-8"),
    ];

    if let Some(value) = expected_value {
        let cfg = DiscoveryConfig::load_from_iter(args).expect(description);
        assert_eq!(cfg.value, value, "{description}");
    } else {
        let err = DiscoveryConfig::load_from_iter(args).expect_err(description);
        assert!(matches!(&*err, OrthoError::File { .. }), "{description}");
    }
}

#[rstest]
fn env_var_overrides_default_locations() {
    let _env = setup_clean_env();
    let dir = TempDir::new().expect("temp dir");
    let config_path = create_test_config(dir.path(), "env.toml", 99);

    let guard = test_env::set_var("APP_CONFIG_PATH", &config_path);
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load config from env");
    drop(guard);

    assert_eq!(cfg.value, 99);
}

#[rstest]
fn xdg_config_home_respects_custom_file_name() {
    let _env = setup_clean_env();
    let dir = TempDir::new().expect("temp dir");
    let xdg_home = dir.path().join("xdg");
    let app_dir = xdg_home.join("demo_app");
    let _ = create_test_config(&app_dir, "demo.toml", 64);

    let guard = test_env::set_var("XDG_CONFIG_HOME", &xdg_home);
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load config from xdg");
    drop(guard);

    assert_eq!(cfg.value, 64);
}

#[rstest]
fn xdg_config_home_missing_file_returns_default() {
    let _env = setup_clean_env();
    let dir = TempDir::new().expect("temp dir");
    let xdg_home = dir.path().join("xdg_missing");
    fs::create_dir_all(&xdg_home).expect("create xdg home");

    let guard = test_env::set_var("XDG_CONFIG_HOME", &xdg_home);
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load default when xdg missing");
    drop(guard);

    assert_eq!(cfg.value, 1);
}

#[rstest]
fn dotfile_fallback_uses_custom_name() {
    let _env = setup_clean_env();
    let dir = TempDir::new().expect("temp dir");
    let _ = create_test_config(dir.path(), ".demo.toml", 23);

    let original_dir = env::current_dir().expect("current dir");
    let _cwd_guard = CwdGuard {
        original: original_dir,
    };
    env::set_current_dir(dir.path()).expect("set current dir");
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load config from dotfile");

    assert_eq!(cfg.value, 23);
}

#[rstest]
fn defaults_apply_when_no_config_found() {
    let _env = setup_clean_env();
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load default config");
    assert_eq!(cfg.value, 1);
}

#[rstest]
fn error_on_malformed_config() {
    let _env = setup_clean_env();
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("broken.toml");
    write_file(&config_path, "value = ???");

    let guard = test_env::set_var("APP_CONFIG_PATH", &config_path);
    let err = DiscoveryConfig::load_from_iter(["prog"]).expect_err("malformed config should fail");
    drop(guard);

    assert!(matches!(&*err, OrthoError::File { .. }));
}
