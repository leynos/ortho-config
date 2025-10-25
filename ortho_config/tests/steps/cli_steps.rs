//! Steps verifying CLI precedence over environment variables and files.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

fn with_jail_loader<F>(world: &mut World, setup: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<Vec<String>>,
{
    let mut result = None;
    figment::Jail::try_with(|j| {
        let args = setup(j)?;
        result = Some(RulesConfig::load_from_iter(args));
        Ok(())
    })
    .map_err(anyhow::Error::new)?;
    ensure!(
        result.is_some(),
        "configuration load did not produce a result"
    );
    world.result = result;
    Ok(())
}

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
fn load_with_cli(world: &mut World, cli: String) -> Result<()> {
    let file_val = world.file_value.clone();
    let env_val = world.env_value.clone();
    with_jail_loader(world, move |j| {
        if let Some(value) = file_val.as_ref() {
            j.create_file(".ddlint.toml", &format!("rules = [\"{value}\"]"))?;
        }
        if let Some(value) = env_val.as_ref() {
            j.set_env("DDLINT_RULES", value);
        }
        Ok(vec!["prog".to_owned(), "--rules".to_owned(), cli])
    })
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
