//! Steps verifying CLI precedence over environment variables and files.

use super::value_parsing::normalize_scalar;
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

fn with_jail_loader<F>(rules_context: &RulesContext, setup: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<ortho_config::OrthoResult<RulesConfig>>,
{
    let config_result = figment_helpers::with_jail(setup)?;
    rules_context.result.set(config_result);
    Ok(())
}

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
    with_jail_loader(rules_context, move |j| {
        if let Some(value) = file_val.as_ref() {
            j.create_file(".ddlint.toml", &format!("rules = [\"{value}\"]"))?;
        }
        if let Some(value) = env_val.as_ref() {
            j.set_env("DDLINT_RULES", value);
        }
        Ok(RulesConfig::load_from_iter([
            "prog",
            "--rules",
            cli_rules.as_str(),
        ]))
    })
}

#[then("the loaded rules are {expected}")]
fn loaded_rules(rules_context: &RulesContext, expected: String) -> Result<()> {
    let expected = normalize_scalar(&expected);
    let result = rules_context
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
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
