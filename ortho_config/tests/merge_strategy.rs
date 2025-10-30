//! Tests for collection merge strategies on vectors and maps.
use anyhow::{Result, anyhow, ensure};
use figment::Jail;
use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::marker::PhantomData;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

/// Helper to load a config and convert figment errors to anyhow errors.
fn load_config<T: OrthoConfig>(args: &[&str]) -> Result<T> {
    T::load_from_iter(args).map_err(|err| anyhow!(err))
}

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

trait HasValues {
    fn values(&self) -> &[String];
}

impl HasValues for VecConfig {
    fn values(&self) -> &[String] {
        &self.values
    }
}

impl HasValues for DefaultVec {
    fn values(&self) -> &[String] {
        &self.values
    }
}

impl HasValues for EmptyVec {
    fn values(&self) -> &[String] {
        &self.values
    }
}

impl HasValues for ReplaceVec {
    fn values(&self) -> &[String] {
        &self.values
    }
}

fn configure_layered_sources(j: &mut Jail) -> Result<()> {
    j.create_file(".config.toml", "values = [\"file\"]")?;
    j.set_env("VALUES", "[\"env\"]");
    Ok(())
}

fn noop_setup(_: &mut Jail) -> Result<()> {
    Ok(())
}

fn run_vector_case<T, Setup>(
    args: &[&str],
    setup: Setup,
    expected: &[&str],
    context: &str,
) -> Result<()>
where
    T: OrthoConfig + HasValues,
    Setup: Fn(&mut Jail) -> Result<()>,
{
    with_jail(|j| {
        setup(j)?;
        let cfg = load_config::<T>(args)?;
        let expected_vec = expected
            .iter()
            .map(|&value| value.to_owned())
            .collect::<Vec<_>>();
        ensure!(
            cfg.values() == expected_vec.as_slice(),
            "{}: expected {:?}, got {:?}",
            context,
            expected_vec,
            cfg.values()
        );
        Ok(())
    })
}

const BASE_ARGS: &[&str] = &["prog"];
const LAYERED_ARGS: &[&str] = &["prog", "--values", "cli1", "--values", "cli2"];
const DEFAULT_APPEND_ARGS: &[&str] = &["prog", "--values", "cli"];

#[rstest]
#[case::append_all_sources(
    PhantomData::<VecConfig>,
    LAYERED_ARGS,
    configure_layered_sources as fn(&mut Jail) -> Result<()>,
    &["file", "env", "cli1", "cli2"],
    "append strategy should retain contributions from every source"
)]
#[case::append_empty(
    PhantomData::<EmptyVec>,
    BASE_ARGS,
    noop_setup,
    &[],
    "append strategy should yield defaults when no layers supply values"
)]
#[case::append_includes_defaults(
    PhantomData::<DefaultVec>,
    DEFAULT_APPEND_ARGS,
    configure_layered_sources as fn(&mut Jail) -> Result<()>,
    &["def", "file", "env", "cli"],
    "append strategy should prepend defaults before layered contributions"
)]
#[case::replace_latest_layer(
    PhantomData::<ReplaceVec>,
    LAYERED_ARGS,
    configure_layered_sources as fn(&mut Jail) -> Result<()>,
    &["cli1", "cli2"],
    "replace strategy should honour highest precedence (CLI) values"
)]
#[case::replace_empty_sources(
    PhantomData::<ReplaceVec>,
    BASE_ARGS,
    noop_setup,
    &[],
    "replace strategy should fall back to defaults when no sources load"
)]
fn vector_merge_strategies<T>(
    #[case] _phantom: PhantomData<T>,
    #[case] args: &[&str],
    #[case] setup: fn(&mut Jail) -> Result<()>,
    #[case] expected: &[&str],
    #[case] context: &str,
) -> Result<()>
where
    T: OrthoConfig + HasValues,
{
    run_vector_case::<T, _>(args, setup, expected, context)
}

#[rstest]
fn replace_maps_drop_lower_precedence_entries() -> Result<()> {
    with_jail(|j| {
        j.create_file(
            ".config.toml",
            "[rules.a]\nenabled = true\n[rules.b]\nenabled = false",
        )?;
        j.set_env("RULES__C__ENABLED", "true");
        let cfg = load_config::<ReplaceMap>(&["prog"])?;
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
