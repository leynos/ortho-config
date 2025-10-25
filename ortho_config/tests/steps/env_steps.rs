//! Cucumber step definitions for environment variable testing.
//!
//! Provides BDD steps for setting environment variables, loading configuration
//! using [`CsvEnv`], and verifying parsed results.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

/// Sets `DDLINT_RULES` in the test environment.
#[given(expr = "the environment variable DDLINT_RULES is {string}")]
fn set_env(world: &mut World, val: String) -> Result<()> {
    ensure!(
        !val.trim().is_empty(),
        "environment rule value must not be empty"
    );
    ensure!(
        world.env_value.is_none(),
        "environment rule value already initialised"
    );
    world.env_value = Some(val);
    Ok(())
}

#[when("the configuration is loaded")]
fn load_config(world: &mut World) -> Result<()> {
    let val = world
        .env_value
        .clone()
        .ok_or_else(|| anyhow!("environment value not configured"))?;
    let mut result = None;
    figment::Jail::try_with(|j| {
        j.set_env("DDLINT_RULES", &val);
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    world.result = result;
    ensure!(
        world.result.is_some(),
        "configuration load did not produce a result"
    );
    Ok(())
}

#[then(expr = "the rules are {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
/// Verifies that the parsed rule list matches the expected string.
fn check_rules(world: &mut World, expected: String) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result.map_err(|err| anyhow!(err))?;
    let want: Vec<String> = expected.split(',').map(str::to_owned).collect();
    ensure!(
        cfg.rules == want,
        "unexpected rules {:?}; expected {:?}",
        cfg.rules,
        want
    );
    Ok(())
}
