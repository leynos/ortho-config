//! Tests for dynamic table deserialization into maps.
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use ortho_config::{CsvEnv, MapEnv};
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Deserialize)]
struct TableConfig {
    #[serde(default)]
    rules: BTreeMap<String, RuleCfg>,
}

#[derive(Debug, Deserialize)]
struct RuleCfg {
    enabled: bool,
}

/// Write a TOML fixture below the caller's temporary directory.
fn write_config_fixture(directory: &Path, contents: &str) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(directory, ambient_authority())
        .context("open dynamic-table fixture directory")?;
    cap.write(".config.toml", contents.as_bytes())
        .context("write dynamic-table fixture")?;
    Ok(directory.join(".config.toml"))
}

/// Asserts that `TableConfig` contains two rules, `a` enabled and `b` disabled.
fn assert_basic_rules(cfg: &TableConfig) -> Result<()> {
    ensure!(
        cfg.rules.get("a").is_some_and(|r| r.enabled),
        "expected rule 'a' to be enabled"
    );
    ensure!(
        cfg.rules.get("b").is_some_and(|r| !r.enabled),
        "expected rule 'b' to be disabled"
    );
    ensure!(
        cfg.rules.len() == 2,
        "unexpected rule entries parsed: {:?}",
        cfg.rules
    );
    Ok(())
}

#[rstest]
#[case::file("file")]
#[case::env("env")]
#[case::cli("cli")]
fn loads_map_from_source(#[case] source: &str) -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create dynamic-table fixture directory")?;
    let fig = match source {
        "file" => {
            let config_path = write_config_fixture(
                temp_dir.path(),
                r"[rules.a]
enabled = true
[rules.b]
enabled = false
",
            )?;
            Figment::from(Toml::file(config_path))
        }
        "env" => {
            let env_source = Arc::new(
                MapEnv::new()
                    .with_var("DDLINT_RULES__A__ENABLED", "true")
                    .with_var("DDLINT_RULES__B__ENABLED", "false"),
            );
            Figment::from(
                CsvEnv::prefixed("DDLINT_")
                    .split("__")
                    .with_source(env_source),
            )
        }
        "cli" => Figment::from(Serialized::defaults(&json!({
            "rules": {
                "a": { "enabled": true },
                "b": { "enabled": false }
            }
        }))),
        other => return Err(anyhow!("unknown source: {other}")),
    };
    let cfg: TableConfig = fig.extract().map_err(|err| anyhow!(err))?;
    assert_basic_rules(&cfg)?;
    Ok(())
}

#[rstest]
fn merges_map_from_sources() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create dynamic-table fixture directory")?;
    let config_path = write_config_fixture(
        temp_dir.path(),
        r"[rules.a]
enabled = true
",
    )?;
    let source = Arc::new(MapEnv::new().with_var("RULES__B__ENABLED", "false"));
    let fig = Figment::from(Toml::file(config_path))
        .merge(CsvEnv::raw().split("__").with_source(source))
        .merge(Serialized::defaults(&json!({
            "rules": { "c": { "enabled": true } }
        })));
    let cfg: TableConfig = fig.extract().map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.rules.get("a").is_some_and(|r| r.enabled),
        "rule a must be enabled"
    );
    ensure!(
        cfg.rules.get("b").is_some_and(|r| !r.enabled),
        "rule b must be disabled"
    );
    ensure!(
        cfg.rules.get("c").is_some_and(|r| r.enabled),
        "rule c must be enabled"
    );
    ensure!(
        cfg.rules.len() == 3,
        "unexpected rule entries parsed: {:?}",
        cfg.rules
    );
    Ok(())
}
