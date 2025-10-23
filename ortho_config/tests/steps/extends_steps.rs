//! Steps for testing configuration inheritance.

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[given("a configuration file extending a base file")]
fn create_files(world: &mut World) -> Result<()> {
    ensure!(!world.extends, "extended configuration already initialised");
    world.extends = true;
    Ok(())
}

#[given("a configuration file with cyclic inheritance")]
fn create_cyclic(world: &mut World) -> Result<()> {
    ensure!(!world.cyclic, "cyclic configuration already initialised");
    world.cyclic = true;
    Ok(())
}

#[given("a configuration file extending a missing base file")]
fn create_missing_base(world: &mut World) -> Result<()> {
    ensure!(
        !world.missing_base,
        "missing-base configuration already initialised"
    );
    world.missing_base = true;
    Ok(())
}

#[when("the extended configuration is loaded")]
fn load_extended(world: &mut World) -> Result<()> {
    let extends = world.extends;
    let mut result = None;
    figment::Jail::try_with(|j| {
        if extends {
            j.create_file("base.toml", "rules = [\"base\"]")?;
            j.create_file(
                ".ddlint.toml",
                "extends = \"base.toml\"\nrules = [\"child\"]",
            )?;
        }
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    world.result = result;
    world.extends = false;
    ensure!(
        world.result.is_some(),
        "extended configuration load did not produce a result"
    );
    Ok(())
}

#[when("the cyclic configuration is loaded")]
fn load_cyclic(world: &mut World) -> Result<()> {
    let mut result = None;
    figment::Jail::try_with(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nrules = [\"a\"]")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nrules = [\"b\"]")?;
        j.create_file(".ddlint.toml", "extends = \"a.toml\"")?;
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    world.result = result;
    world.cyclic = false;
    ensure!(
        world.result.is_some(),
        "cyclic configuration load did not produce a result"
    );
    Ok(())
}

#[when("the configuration with missing base is loaded")]
fn load_missing_base(world: &mut World) -> Result<()> {
    let mut result = None;
    figment::Jail::try_with(|j| {
        j.create_file(
            ".ddlint.toml",
            "extends = \"missing.toml\"\nrules = [\"main\"]",
        )?;
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    world.result = result;
    world.missing_base = false;
    ensure!(
        world.result.is_some(),
        "missing-base configuration load did not produce a result"
    );
    Ok(())
}

#[then("an error occurs")]
fn error_occurs(world: &mut World) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    ensure!(result.is_err(), "expected configuration to fail");
    Ok(())
}
