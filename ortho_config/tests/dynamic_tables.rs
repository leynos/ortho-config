//! Tests for dynamic table deserialization into maps.

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;
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
            r"[rules.a]
enabled = true
[rules.b]
enabled = false
",
        )?;
        let fig = Figment::from(Toml::file(".config.toml"));
        let cfg: TableConfig = fig.extract().expect("extract");
        assert!(cfg.rules.get("a").is_some_and(|r| r.enabled));
        assert!(cfg.rules.get("b").is_some_and(|r| !r.enabled));
        assert_eq!(cfg.rules.len(), 2, "unexpected rule entries parsed");
        Ok(())
    });
}

#[rstest]
fn loads_map_from_env() {
    figment::Jail::expect_with(|j| {
        j.set_env("DDLINT_RULES__A__ENABLED", "true");
        j.set_env("DDLINT_RULES__B__ENABLED", "false");
        let fig = Figment::from(Env::prefixed("DDLINT_").split("__"));
        let cfg: TableConfig = fig.extract().expect("extract");
        assert!(cfg.rules.get("a").is_some_and(|r| r.enabled));
        assert!(cfg.rules.get("b").is_some_and(|r| !r.enabled));
        assert_eq!(cfg.rules.len(), 2, "unexpected rule entries parsed");
        Ok(())
    });
}

#[rstest]
fn loads_map_from_cli() {
    figment::Jail::expect_with(|_j| {
        let fig = Figment::from(Serialized::defaults(&json!({
            "rules": {
                "a": { "enabled": true },
                "b": { "enabled": false }
            }
        })));
        let cfg: TableConfig = fig.extract().expect("extract");
        assert!(cfg.rules.get("a").is_some_and(|r| r.enabled));
        assert!(cfg.rules.get("b").is_some_and(|r| !r.enabled));
        assert_eq!(cfg.rules.len(), 2, "unexpected rule entries parsed");
        Ok(())
    });
}

#[rstest]
fn merges_map_from_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file(
            ".config.toml",
            r"[rules.a]
enabled = true
",
        )?;
        j.set_env("RULES__B__ENABLED", "false");
        let fig = Figment::from(Toml::file(".config.toml"))
            .merge(Env::raw().split("__"))
            .merge(Serialized::defaults(&json!({
                "rules": { "c": { "enabled": true } }
            })));
        let cfg: TableConfig = fig.extract().expect("extract");
        assert!(cfg.rules.get("a").is_some_and(|r| r.enabled));
        assert!(cfg.rules.get("b").is_some_and(|r| !r.enabled));
        assert!(cfg.rules.get("c").is_some_and(|r| r.enabled));
        assert_eq!(cfg.rules.len(), 3, "unexpected rule entries parsed");
        Ok(())
    });
}
