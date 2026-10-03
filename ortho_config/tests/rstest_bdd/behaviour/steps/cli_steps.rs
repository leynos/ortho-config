//! Steps verifying CLI precedence over environment variables and files.

use super::common::{SlotTakeOrExt, config_fixture, shared_sources};
use super::value_parsing::normalize_scalar;
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig};
use rstest_bdd_macros::{given, then, when};

#[given("the configuration file has rules {value}")]
fn file_rules(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    ensure!(
        !value.trim().is_empty(),
        "configuration rule value must not be empty"
    );
    ensure!(
        rules_context.file_value.is_empty(),
        "configuration file rule already initialised"
    );
    rules_context.file_value.set(value);
    Ok(())
}

#[when("the config is loaded with CLI rules {cli_rules}")]
fn load_with_cli(rules_context: &RulesContext, cli_rules: String) -> Result<()> {
    let cli_rules = normalize_scalar(&cli_rules);
    let file_val = rules_context.file_value.get();
    let env_val = rules_context.env_value.get();
    let file_contents = file_val
        .map(|value| format!("rules = [\"{value}\"]"))
        .unwrap_or_default();
    // The guard stays bound until the loader below has read the fixture.
    let (_fixture_dir, config_path) = config_fixture(".ddlint.toml", Some(&file_contents))?;
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", config_path);
    if let Some(value) = env_val.as_ref() {
        source = source.with_var("DDLINT_RULES", value);
    }
    let (discovery, merge) = shared_sources(source);
    let config_result = RulesConfig::load_from_iter_with_sources(
        ["prog", "--rules", cli_rules.as_str()],
        discovery,
        merge,
    );
    rules_context.result.set(config_result);
    Ok(())
}

#[then("the loaded rules are {expected}")]
fn loaded_rules(rules_context: &RulesContext, expected: String) -> Result<()> {
    let expected = normalize_scalar(&expected);
    let result = rules_context
        .result
        .take_or("configuration result unavailable")?;
    let cfg = result.map_err(anyhow::Error::from)?;
    let rule = cfg
        .rules
        .last()
        .ok_or_else(|| anyhow!("expected at least one rule"))?;
    ensure!(
        rule == &expected,
        "unexpected rule {rule:?}; expected {expected:?}"
    );
    Ok(())
}
