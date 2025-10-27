//! Tests for collection merge strategies on vectors and maps.
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct VecConfig {
    #[ortho_config(merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct DefaultVec {
    #[ortho_config(default = vec!["def".to_owned()], merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct EmptyVec {
    #[ortho_config(default = vec![], merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ReplaceVec {
    #[serde(default)]
    #[ortho_config(default = vec![], merge_strategy = "replace")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Rule {
    enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ReplaceMap {
    #[serde(default)]
    #[ortho_config(skip_cli, merge_strategy = "replace")]
    rules: BTreeMap<String, Rule>,
}

#[rstest]
fn append_merges_all_sources() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = VecConfig::load_from_iter(["prog", "--values", "cli1", "--values", "cli2"])
            .map_err(|err| anyhow!(err))?;
        let expected = vec!["file", "env", "cli1", "cli2"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        ensure!(
            cfg.values == expected,
            "expected {:?}, got {:?}",
            expected,
            cfg.values
        );
        Ok(())
    })
}

#[rstest]
fn append_empty_sources_yields_empty() -> Result<()> {
    with_jail(|_| {
        let cfg = EmptyVec::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.values.is_empty(),
            "expected empty values, got {:?}",
            cfg.values
        );
        Ok(())
    })
}

#[rstest]
fn append_includes_defaults() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg =
            DefaultVec::load_from_iter(["prog", "--values", "cli"]).map_err(|err| anyhow!(err))?;
        let expected = vec!["def", "file", "env", "cli"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        ensure!(
            cfg.values == expected,
            "expected {:?}, got {:?}",
            expected,
            cfg.values
        );
        Ok(())
    })
}

#[rstest]
fn replace_vectors_take_latest_layer() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = ReplaceVec::load_from_iter(["prog", "--values", "cli1", "--values", "cli2"])
            .map_err(|err| anyhow!(err))?;
        let expected = vec![String::from("cli1"), String::from("cli2")];
        ensure!(
            cfg.values == expected,
            "expected {:?}, got {:?}",
            expected,
            cfg.values
        );
        Ok(())
    })
}

#[rstest]
fn replace_vectors_empty_when_no_layers() -> Result<()> {
    with_jail(|_| {
        let cfg = ReplaceVec::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.values.is_empty(),
            "expected empty values for replace with no sources"
        );
        Ok(())
    })
}

#[rstest]
fn replace_vectors_default_to_empty_when_unset() -> Result<()> {
    with_jail(|_| {
        let cfg = ReplaceVec::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.values.is_empty(),
            "expected empty values when replace strategy receives no inputs, got {:?}",
            cfg.values
        );
        Ok(())
    })
}

#[rstest]
fn replace_maps_drop_lower_precedence_entries() -> Result<()> {
    with_jail(|j| {
        j.create_file(
            ".config.toml",
            "[rules.a]\nenabled = true\n[rules.b]\nenabled = false",
        )?;
        j.set_env("RULES__C__ENABLED", "true");
        let cfg = ReplaceMap::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.rules.get("c").is_some_and(|rule| rule.enabled),
            "expected rule c to be enabled"
        );
        ensure!(
            !cfg.rules.contains_key("a"),
            "rule a from file should be replaced by higher precedence"
        );
        ensure!(
            !cfg.rules.contains_key("b"),
            "rule b from file should be replaced by higher precedence"
        );
        Ok(())
    })
}
