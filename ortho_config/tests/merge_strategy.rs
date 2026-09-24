//! Tests for collection merge strategies on vectors and maps.
use anyhow::{Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Helper to load a config and convert figment errors to anyhow errors.
fn load_config<T: OrthoConfig>(args: Vec<OsString>, env: MapEnv) -> Result<T> {
    let source = Arc::new(env);
    T::load_from_iter_with_sources(args, source.clone(), source).map_err(|err| anyhow!(err))
}

fn write_config(dir: &Path, contents: &str) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(dir, ambient_authority())?;
    cap.write("config.toml", contents.as_bytes())?;
    Ok(dir.join("config.toml"))
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

fn run_vector_case<T, Setup>(
    args: &[&str],
    setup: Setup,
    expected: &[&str],
    context: &str,
) -> Result<()>
where
    T: OrthoConfig + HasValues,
    Setup: Fn(&Path) -> Result<(MapEnv, Option<PathBuf>)>,
{
    let temp_dir = tempfile::tempdir()?;
    let (env, file) = setup(temp_dir.path())?;
    let mut resolved_args: Vec<OsString> = args.iter().map(|arg| OsString::from(*arg)).collect();
    if let Some(path) = file {
        resolved_args.push("--config-path".into());
        resolved_args.push(path.into_os_string());
    }
    let cfg = load_config::<T>(resolved_args, env)?;
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
}

fn configure_layered_sources(dir: &Path) -> Result<(MapEnv, Option<PathBuf>)> {
    let path = write_config(dir, "values = [\"file\"]")?;
    Ok((MapEnv::new().with_var("VALUES", "[\"env\"]"), Some(path)))
}

const BASE_ARGS: &[&str] = &["prog"];
const LAYERED_ARGS: &[&str] = &["prog", "--values", "cli1", "--values", "cli2"];
const DEFAULT_APPEND_ARGS: &[&str] = &["prog", "--values", "cli"];

#[rstest]
#[case::append_all_sources(case_append_all_sources)]
#[case::append_empty(case_append_empty)]
#[case::append_includes_defaults(case_append_includes_defaults)]
#[case::replace_latest_layer(case_replace_latest_layer)]
#[case::replace_empty_sources(case_replace_empty_sources)]
fn vector_merge_strategies(#[case] scenario: fn() -> Result<()>) -> Result<()> {
    scenario()
}

fn case_append_all_sources() -> Result<()> {
    run_vector_case::<VecConfig, _>(
        LAYERED_ARGS,
        configure_layered_sources,
        &["file", "env", "cli1", "cli2"],
        "append strategy should retain contributions from every source",
    )
}

fn case_append_empty() -> Result<()> {
    run_vector_case::<EmptyVec, _>(
        BASE_ARGS,
        |_| Ok((MapEnv::new(), None)),
        &[],
        "append strategy should yield defaults when no layers supply values",
    )
}

fn case_append_includes_defaults() -> Result<()> {
    run_vector_case::<DefaultVec, _>(
        DEFAULT_APPEND_ARGS,
        configure_layered_sources,
        &["def", "file", "env", "cli"],
        "append strategy should prepend defaults before layered contributions",
    )
}

fn case_replace_latest_layer() -> Result<()> {
    run_vector_case::<ReplaceVec, _>(
        LAYERED_ARGS,
        configure_layered_sources,
        &["cli1", "cli2"],
        "replace strategy should honour highest precedence (CLI) values",
    )
}

fn case_replace_empty_sources() -> Result<()> {
    run_vector_case::<ReplaceVec, _>(
        BASE_ARGS,
        |_| Ok((MapEnv::new(), None)),
        &[],
        "replace strategy should fall back to defaults when no sources load",
    )
}

#[rstest]
fn replace_maps_drop_lower_precedence_entries() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = write_config(
        temp_dir.path(),
        "[rules.a]\nenabled = true\n[rules.b]\nenabled = false",
    )?;
    let cfg = load_config::<ReplaceMap>(
        vec!["prog".into(), "--config-path".into(), path.into_os_string()],
        MapEnv::new().with_var("RULES__C__ENABLED", "true"),
    )?;
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
}
