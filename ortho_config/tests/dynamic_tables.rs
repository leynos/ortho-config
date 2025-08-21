//! Tests for dynamic table deserialization into maps.

use figment::{Figment, providers::Format, providers::Toml};
use rstest::rstest;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct TableConfig {
    #[serde(default)]
    rules: BTreeMap<String, RuleCfg>,
}

#[derive(Debug, Deserialize)]
struct RuleCfg {
    enabled: bool,
}

#[rstest]
fn loads_map_from_file() {
    figment::Jail::expect_with(|j| {
        j.create_file(
            ".config.toml",
            "[rules.a]\nenabled = true\n[rules.b]\nenabled = false",
        )?;
        let fig = Figment::from(Toml::file(".config.toml"));
        let cfg: TableConfig = fig.extract().expect("extract");
        assert!(cfg.rules.get("a").is_some_and(|r| r.enabled));
        assert!(cfg.rules.get("b").is_some_and(|r| !r.enabled));
        Ok(())
    });
}
