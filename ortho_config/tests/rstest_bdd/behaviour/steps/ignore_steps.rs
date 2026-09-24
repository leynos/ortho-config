//! Steps for testing ignore pattern list handling.

use super::common::{SlotTakeOrExt, set_scalar_once};
use super::value_parsing::{normalize_scalar, parse_csv_values};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{given, then, when};
use std::sync::Arc;

#[given("the environment variable DDLINT_IGNORE_PATTERNS is {value}")]
fn set_ignore_env(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_scalar_once(
        &rules_context.env_value,
        value,
        "ignore patterns environment value",
    )
}

#[when("the config is loaded with CLI ignore {cli}")]
fn load_ignore(rules_context: &RulesContext, cli: String) -> Result<()> {
    let cli = normalize_scalar(&cli);
    let env_val = rules_context.env_value.take();
    let fixture_dir = tempfile::tempdir().context("create ignore config fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open ignore config fixture directory")?;
    fixture
        .write(".ddlint.toml", b"")
        .context("write ignore config fixture")?;
    let config_path = fixture_dir.path().join(".ddlint.toml");
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", config_path);
    if let Some(value) = env_val
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        source = source.with_var("DDLINT_IGNORE_PATTERNS", value);
    }
    let source = Arc::new(source);
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let mut args = vec![String::from("prog")];
    if !cli.is_empty() {
        args.push("--ignore-patterns".into());
        args.push(cli.trim().to_owned());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let config_result =
        <RulesConfig as OrthoConfig>::load_from_iter_with_sources(refs, discovery, merge);
    rules_context.result.set(config_result);
    Ok(())
}

#[then("the ignore patterns are {patterns}")]
fn check_ignore(rules_context: &RulesContext, patterns: String) -> Result<()> {
    let result = rules_context
        .result
        .take_or("configuration result unavailable")?;
    let cfg = result.map_err(anyhow::Error::from)?;
    let want = parse_csv_values(&patterns);
    ensure!(
        cfg.ignore_patterns == want,
        "unexpected ignore patterns {:?}; expected {:?}",
        cfg.ignore_patterns,
        want
    );
    Ok(())
}
