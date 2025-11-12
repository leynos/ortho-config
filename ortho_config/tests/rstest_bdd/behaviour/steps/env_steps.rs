//! Step definitions for environment variable testing.
//!
//! Provides BDD steps for setting environment variables, loading configuration
//! using [`CsvEnv`], and verifying parsed results.

use crate::fixtures::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};

/// Sets `DDLINT_RULES` in the test environment.
#[given("the environment variable DDLINT_RULES is {value}")]
fn set_env(rules_context: &RulesContext, value: String) -> Result<()> {
    ensure!(
        !value.trim().is_empty(),
        "environment rule value must not be empty"
    );
    ensure!(
        rules_context.env_value.is_empty(),
        "environment rule value already initialised"
    );
    rules_context.env_value.set(value);
    Ok(())
}

#[when("the configuration is loaded")]
fn load_config(rules_context: &RulesContext) -> Result<()> {
    let value = rules_context
        .env_value
        .get()
        .ok_or_else(|| anyhow!("environment value not configured"))?;
    let mut result = None;
    figment::Jail::try_with(|j| {
        j.set_env("DDLINT_RULES", &value);
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    let config_result =
        result.ok_or_else(|| anyhow!("configuration load did not produce a result"))?;
    rules_context.result.set(config_result);
    Ok(())
}

/// Verifies that the parsed rule list matches the expected string.
#[then("the rules are {rules}")]
fn check_rules(rules_context: &RulesContext, rules: String) -> Result<()> {
    let result = rules_context
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result.map_err(|err| anyhow!(err))?;
    let want: Vec<String> = rules.split(',').map(str::to_owned).collect();
    ensure!(
        cfg.rules == want,
        "unexpected rules {:?}; expected {:?}",
        cfg.rules,
        want
    );
    Ok(())
}
