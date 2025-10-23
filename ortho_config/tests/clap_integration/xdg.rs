//! XDG configuration discovery tests (Unix platforms only).

use super::common::{assert_config_values, with_jail, OrthoResultExt, TestConfig};
use anyhow::{anyhow, Result};

fn run_xdg_case(file_name: &str, contents: &str) -> Result<TestConfig> {
    with_jail(|j| {
        let dir = j.create_dir("xdg")?;
        let abs = ortho_config::file::canonicalise(&dir).to_anyhow()?;
        j.create_file(dir.join(file_name), contents)?;
        let dir_value = abs
            .to_str()
            .ok_or_else(|| anyhow!("canonical path is not valid UTF-8: {:?}", abs))?
            .to_owned();
        j.set_env("XDG_CONFIG_HOME", &dir_value);
        TestConfig::load_from_iter(["prog"]).to_anyhow()
    })
}

#[test]
fn loads_from_xdg_config() -> Result<()> {
    let cfg = run_xdg_case(
        "config.toml",
        "sample_value = \"xdg\"\nother = \"val\"",
    )?;
    assert_config_values(&cfg, Some("xdg"), Some("val"))
}

#[cfg(feature = "yaml")]
#[test]
fn loads_from_xdg_yaml_config() -> Result<()> {
    let cfg = run_xdg_case("config.yaml", "sample_value: xdg\nother: val")?;
    assert_config_values(&cfg, Some("xdg"), Some("val"))
}
