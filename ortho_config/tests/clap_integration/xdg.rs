//! XDG configuration discovery tests (Unix platforms only).

use super::common::{OrthoConfig, TestConfig, ToAnyhow, assert_config_values};
use anyhow::Result;
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::MapEnv;
use std::sync::Arc;

/// Writes `file_name` under a fixture `xdg/` directory and loads through the
/// injected `XDG_CONFIG_HOME`/`XDG_CONFIG_DIRS`; pinning `XDG_CONFIG_DIRS` to
/// an empty fixture directory keeps the XDG list rung from reaching the host's
/// `/etc/xdg`.
fn run_xdg_case(file_name: &str, contents: &str) -> Result<TestConfig> {
    let temp_dir = tempfile::tempdir()?;
    let cap = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())?;
    cap.create_dir("xdg")?;
    cap.write(format!("xdg/{file_name}"), contents.as_bytes())?;
    // Pin `XDG_CONFIG_DIRS` to an empty fixture directory so the XDG *list*
    // rung resolves to a location that matches nothing. Without it the
    // platform default, `/etc/xdg`, would be searched too — which is how this
    // case would otherwise depend on the host.
    let empty_dirs = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())?;
    empty_dirs.create_dir("empty-xdg")?;
    let source = Arc::new(
        MapEnv::new()
            .with_var("XDG_CONFIG_HOME", temp_dir.path().join("xdg"))
            .with_var("XDG_CONFIG_DIRS", temp_dir.path().join("empty-xdg")),
    );
    TestConfig::load_from_iter_with_sources(["prog"], source.clone(), source).to_anyhow()
}

#[test]
fn loads_from_xdg_config() -> Result<()> {
    let cfg = run_xdg_case("config.toml", "sample_value = \"xdg\"\nother = \"val\"")?;
    assert_config_values(&cfg, Some("xdg"), Some("val"))
}

#[cfg(feature = "yaml")]
#[test]
fn loads_from_xdg_yaml_config() -> Result<()> {
    let cfg = run_xdg_case("config.yaml", "sample_value: xdg\nother: val")?;
    assert_config_values(&cfg, Some("xdg"), Some("val"))
}
