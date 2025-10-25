//! Cucumber step definitions for testing subcommand configuration loading and
//! merging behaviour.
//!
//! This module provides step definitions that verify the correct precedence and
//! merging of configuration sources (CLI arguments, environment variables, and
//! configuration files) when loading subcommand configurations.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{PrArgs, World};
use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use cucumber::{given, then, when};
use ortho_config::SubcmdConfigMerge;

/// Check if all configuration sources are absent.
const fn has_no_config_sources(world: &World) -> bool {
    world.sub_ref.is_none() && world.sub_file.is_none() && world.sub_env.is_none()
}

#[given(expr = "a CLI reference {string}")]
fn set_cli_ref(world: &mut World, reference: String) -> Result<()> {
    ensure!(
        !reference.trim().is_empty(),
        "CLI reference must not be empty"
    );
    world.sub_ref = Some(reference);
    Ok(())
}

#[given("no CLI reference")]
fn no_cli_ref(world: &mut World) -> Result<()> {
    ensure!(world.sub_ref.is_none(), "CLI reference already initialised");
    Ok(())
}

#[given(expr = "a configuration reference {string}")]
fn file_ref(world: &mut World, reference: String) -> Result<()> {
    ensure!(
        !reference.trim().is_empty(),
        "configuration file reference must not be empty"
    );
    world.sub_file = Some(reference);
    Ok(())
}

#[given(expr = "an environment reference {string}")]
fn env_ref(world: &mut World, reference: String) -> Result<()> {
    ensure!(
        !reference.trim().is_empty(),
        "environment reference must not be empty"
    );
    world.sub_env = Some(reference);
    Ok(())
}

#[when("the subcommand configuration is loaded without defaults")]
fn load_sub(world: &mut World) -> Result<()> {
    let result = if has_no_config_sources(world) {
        PrArgs::try_parse_from(["test"]).map_err(anyhow::Error::from)
    } else {
        let cli = PrArgs {
            reference: world.sub_ref.clone(),
        };
        setup_test_environment(world, &cli)?
    };
    world.sub_result = Some(result);
    world.sub_file = None;
    world.sub_env = None;
    Ok(())
}

/// Set up test environment with configuration file and environment variables.
fn setup_test_environment(world: &World, cli: &PrArgs) -> Result<Result<PrArgs, anyhow::Error>> {
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(file_reference) = world.sub_file.as_ref() {
            j.create_file(
                ".app.toml",
                &format!("[cmds.test]\nreference = \"{file_reference}\""),
            )?;
        }
        if let Some(env_reference) = world.sub_env.as_ref() {
            j.set_env("APP_CMDS_TEST_REFERENCE", env_reference);
        }
        result = Some(cli.load_and_merge().map_err(anyhow::Error::from));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    result.ok_or_else(|| anyhow!("subcommand merge did not run within figment jail"))
}

#[then(expr = "the merged reference is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn check_ref(world: &mut World, expected_reference: String) -> Result<()> {
    let result = world
        .sub_result
        .take()
        .ok_or_else(|| anyhow!("subcommand result unavailable"))?;
    let cfg = result?;
    ensure!(
        cfg.reference.as_deref() == Some(expected_reference.as_str()),
        "unexpected reference {:?}; expected {:?}",
        cfg.reference,
        expected_reference
    );
    Ok(())
}

#[then("the subcommand load fails")]
fn sub_error(world: &mut World) -> Result<()> {
    let result = world
        .sub_result
        .take()
        .ok_or_else(|| anyhow!("subcommand result unavailable"))?;
    ensure!(result.is_err(), "expected subcommand load to fail");
    Ok(())
}
