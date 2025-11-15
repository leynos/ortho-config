//! Step definitions for environment variable testing.
//!
//! Provides BDD steps for setting environment variables, loading configuration
//! using [`CsvEnv`], and verifying parsed results.

use crate::fixtures::{RulesConfig, RulesContext};
use anyhow::{Context, Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

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

/// Loads configuration solely from the `DDLINT_RULES` environment value.
#[when("the configuration is loaded")]
fn load_config(rules_context: &RulesContext) -> Result<()> {
    let value = rules_context
        .env_value
        .get()
        .ok_or_else(|| anyhow!("environment value not configured"))?;
    let config_result = figment_helpers::with_jail(|j| {
        j.set_env("DDLINT_RULES", &value);
        Ok(RulesConfig::load_from_iter(["prog"]))
    })
    .context("failed to load rules configuration from jailed environment")?;
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
    let cfg = result.context("failed to parse rules configuration")?;
    let want: Vec<String> = rules.split(',').map(|s| s.trim().to_owned()).collect();
    ensure!(
        cfg.rules == want,
        "unexpected rules {:?}; expected {:?}",
        cfg.rules,
        want
    );
    Ok(())
}
