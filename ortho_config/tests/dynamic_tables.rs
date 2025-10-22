//! Tests for dynamic table deserialization into maps.
use anyhow::{Result, anyhow, ensure};
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

fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err))
}

/// Asserts that `TableConfig` contains two rules, `a` enabled and `b` disabled.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use crate::{RuleCfg, TableConfig};
///
/// let cfg = TableConfig {
///     rules: BTreeMap::from([
///         ("a".into(), RuleCfg { enabled: true }),
///         ("b".into(), RuleCfg { enabled: false }),
///     ]),
/// };
/// assert_basic_rules(&cfg).unwrap();
/// ```
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
    with_jail(|j| {
        let fig = match source {
            "file" => {
                j.create_file(
                    ".config.toml",
                    r"[rules.a]
enabled = true
[rules.b]
enabled = false
",
                )?;
                Figment::from(Toml::file(".config.toml"))
            }
            "env" => {
                j.set_env("DDLINT_RULES__A__ENABLED", "true");
                j.set_env("DDLINT_RULES__B__ENABLED", "false");
                Figment::from(Env::prefixed("DDLINT_").split("__"))
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
    })?;
    Ok(())
}

#[rstest]
fn merges_map_from_sources() -> Result<()> {
    with_jail(|j| {
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
    })?;
    Ok(())
}
