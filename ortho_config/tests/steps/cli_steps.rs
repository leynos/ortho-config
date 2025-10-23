//! Steps verifying CLI precedence over environment variables and files.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[given(expr = "the configuration file has rules {string}")]
fn file_rules(world: &mut World, val: String) -> Result<()> {
    ensure!(
        !val.trim().is_empty(),
        "configuration rule value must not be empty"
    );
    ensure!(
        world.file_value.is_none(),
        "configuration file rule already initialised"
    );
    world.file_value = Some(val);
    Ok(())
}

#[when(expr = "the config is loaded with CLI rules {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn load_with_cli(world: &mut World, cli: String) -> Result<()> {
    let file_val = world.file_value.clone();
    let env_val = world.env_value.clone();
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(value) = file_val.as_ref() {
            j.create_file(".ddlint.toml", &format!("rules = [\"{value}\"]"))?;
        }
        if let Some(value) = env_val.as_ref() {
            j.set_env("DDLINT_RULES", value);
        }
        result = Some(RulesConfig::load_from_iter([
            "prog",
            "--rules",
            cli.as_str(),
        ]));
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

#[then(expr = "the loaded rules are {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn loaded_rules(world: &mut World, expected: String) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result.map_err(|err| anyhow!(err))?;
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
