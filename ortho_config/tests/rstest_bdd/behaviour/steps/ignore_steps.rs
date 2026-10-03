//! Steps for testing ignore pattern list handling.

use super::common::{SlotTakeOrExt, config_fixture, set_scalar_once, shared_sources};
use super::value_parsing::{normalize_scalar, parse_csv_values};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, ensure};
use ortho_config::{MapEnv, OrthoConfig};
use rstest_bdd_macros::{given, then, when};

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
    // The guard stays bound until the loader below has read the fixture.
    let (_fixture_dir, config_path) = config_fixture(".ddlint.toml", Some(""))?;
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", config_path);
    if let Some(value) = env_val
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        source = source.with_var("DDLINT_IGNORE_PATTERNS", value);
    }
    let (discovery, merge) = shared_sources(source);
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
