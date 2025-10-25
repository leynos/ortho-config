//! Steps for testing configuration inheritance.

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::{OrthoConfig, OrthoResult};

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

fn with_jail_load<F>(setup: F) -> Result<OrthoResult<RulesConfig>>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    let mut output = None;
    figment::Jail::try_with(|j| {
        setup(j)?;
        output = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err))?;
    output.ok_or_else(|| anyhow!("loader did not run"))
}

#[when("the extended configuration is loaded")]
fn load_extended(world: &mut World) -> Result<()> {
    let extends = world.extends;
    world.result = Some(with_jail_load(|j| {
        if extends {
            j.create_file("base.toml", "rules = [\"base\"]")?;
            j.create_file(
                ".ddlint.toml",
                "extends = \"base.toml\"\nrules = [\"child\"]",
            )?;
        }
        Ok(())
    })?);
    world.extends = false;
    Ok(())
}

#[when("the cyclic configuration is loaded")]
fn load_cyclic(world: &mut World) -> Result<()> {
    world.result = Some(with_jail_load(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nrules = [\"a\"]")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nrules = [\"b\"]")?;
        j.create_file(".ddlint.toml", "extends = \"a.toml\"")?;
        Ok(())
    })?);
    world.cyclic = false;
    Ok(())
}

#[when("the configuration with missing base is loaded")]
fn load_missing_base(world: &mut World) -> Result<()> {
    world.result = Some(with_jail_load(|j| {
        j.create_file(
            ".ddlint.toml",
            "extends = \"missing.toml\"\nrules = [\"main\"]",
        )?;
        Ok(())
    })?);
    world.missing_base = false;
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
