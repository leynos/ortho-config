//! Steps for testing configuration inheritance.

use crate::fixtures::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoResult};
use rstest_bdd_macros::{given, then, when};

#[given("a configuration file extending a base file")]
fn create_files(world: &World) -> Result<()> {
    ensure!(
        world.extends_flag.is_empty(),
        "extended configuration already initialised"
    );
    world.extends_flag.set(());
    Ok(())
}

#[given("a configuration file with cyclic inheritance")]
fn create_cyclic(world: &World) -> Result<()> {
    ensure!(
        world.cyclic_flag.is_empty(),
        "cyclic configuration already initialised"
    );
    world.cyclic_flag.set(());
    Ok(())
}

#[given("a configuration file extending a missing base file")]
fn create_missing_base(world: &World) -> Result<()> {
    ensure!(
        world.missing_base_flag.is_empty(),
        "missing-base configuration already initialised"
    );
    world.missing_base_flag.set(());
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
fn load_extended(world: &World) -> Result<()> {
    ensure!(
        world.extends_flag.is_filled(),
        "extended configuration was not initialised"
    );
    world.extends_flag.clear();
    let result = with_jail_load(|j| {
        j.create_file("base.toml", "rules = [\"base\"]")?;
        j.create_file(
            ".ddlint.toml",
            "extends = \"base.toml\"\nrules = [\"child\"]",
        )?;
        Ok(())
    })?;
    world.result.set(result);
    Ok(())
}

#[when("the cyclic configuration is loaded")]
fn load_cyclic(world: &World) -> Result<()> {
    ensure!(
        world.cyclic_flag.is_filled(),
        "cyclic configuration was not initialised"
    );
    world.cyclic_flag.clear();
    let result = with_jail_load(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nrules = [\"a\"]")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nrules = [\"b\"]")?;
        j.create_file(".ddlint.toml", "extends = \"a.toml\"")?;
        Ok(())
    })?;
    world.result.set(result);
    Ok(())
}

#[when("the configuration with missing base is loaded")]
fn load_missing_base(world: &World) -> Result<()> {
    ensure!(
        world.missing_base_flag.is_filled(),
        "missing-base configuration was not initialised"
    );
    world.missing_base_flag.clear();
    let result = with_jail_load(|j| {
        j.create_file(
            ".ddlint.toml",
            "extends = \"missing.toml\"\nrules = [\"main\"]",
        )?;
        Ok(())
    })?;
    world.result.set(result);
    Ok(())
}

#[then("an error occurs")]
fn error_occurs(world: &World) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    ensure!(result.is_err(), "expected configuration to fail");
    Ok(())
}
