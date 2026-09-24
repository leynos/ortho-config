//! XDG configuration discovery tests (Unix platforms only).

use super::common::{OrthoConfig, TestConfig, ToAnyhow, assert_config_values};
use anyhow::Result;
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::MapEnv;
use std::sync::Arc;

fn run_xdg_case(file_name: &str, contents: &str) -> Result<TestConfig> {
    let temp_dir = tempfile::tempdir()?;
    let cap = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())?;
    cap.create_dir("xdg")?;
    cap.write(format!("xdg/{file_name}"), contents.as_bytes())?;
    let source = Arc::new(MapEnv::new().with_var("XDG_CONFIG_HOME", temp_dir.path().join("xdg")));
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
