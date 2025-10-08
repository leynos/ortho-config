//! Tests for struct-level discovery attributes.

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

#[rstest]
fn cli_long_flag_loads_config() {
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("explicit.toml");
    write_file(&config_path, "value = 41");

    let cfg = DiscoveryConfig::load_from_iter([
        "prog",
        "--config",
        config_path.to_str().expect("utf8 path"),
    ])
    .expect("load config from cli path");

    assert_eq!(cfg.value, 41);
}

#[rstest]
fn cli_long_flag_invalid_config_path() {
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("missing.toml");

    let err = DiscoveryConfig::load_from_iter([
        "prog",
        "--config",
        config_path.to_str().expect("utf8 path"),
    ])
    .expect_err("error when CLI path missing");

    assert!(matches!(&*err, OrthoError::File { .. }));
}

#[rstest]
fn cli_short_flag_loads_config() {
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("short.toml");
    write_file(&config_path, "value = 17");

    let cfg =
        DiscoveryConfig::load_from_iter(["prog", "-c", config_path.to_str().expect("utf8 path")])
            .expect("load config from short flag");

    assert_eq!(cfg.value, 17);
}

#[rstest]
fn env_var_overrides_default_locations() {
    let _env = clear_support_env();
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("env.toml");
    write_file(&config_path, "value = 99");

    let guard = test_env::set_var("APP_CONFIG_PATH", &config_path);
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load config from env");
    drop(guard);

    assert_eq!(cfg.value, 99);
}

#[rstest]
fn xdg_config_home_respects_custom_file_name() {
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
    let dir = TempDir::new().expect("temp dir");
    let xdg_home = dir.path().join("xdg");
    let xdg_file = xdg_home.join("demo_app").join("demo.toml");
    write_file(&xdg_file, "value = 64");

    let guard = test_env::set_var("XDG_CONFIG_HOME", &xdg_home);
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load config from xdg");
    drop(guard);

    assert_eq!(cfg.value, 64);
}

#[rstest]
fn xdg_config_home_missing_file_returns_default() {
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
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
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
    let dir = TempDir::new().expect("temp dir");
    let dotfile = dir.path().join(".demo.toml");
    write_file(&dotfile, "value = 23");

    let original_dir = env::current_dir().expect("current dir");
    env::set_current_dir(dir.path()).expect("set current dir");
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load config from dotfile");
    env::set_current_dir(original_dir).expect("restore current dir");

    assert_eq!(cfg.value, 23);
}

#[rstest]
fn defaults_apply_when_no_config_found() {
    let _env = clear_support_env();
    let _config = test_env::remove_var("APP_CONFIG_PATH");
    let cfg = DiscoveryConfig::load_from_iter(["prog"]).expect("load default config");
    assert_eq!(cfg.value, 1);
}

#[rstest]
fn error_on_malformed_config() {
    let _env = clear_support_env();
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("broken.toml");
    write_file(&config_path, "value = ???");

    let guard = test_env::set_var("APP_CONFIG_PATH", &config_path);
    let err = DiscoveryConfig::load_from_iter(["prog"]).expect_err("malformed config should fail");
    drop(guard);

    assert!(matches!(&*err, OrthoError::File { .. }));
}
