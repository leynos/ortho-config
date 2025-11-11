//! Step definitions for testing subcommand configuration loading and merging.
//!
//! This module verifies the precedence of CLI, environment, and configuration
//! file sources when loading subcommand inputs.

use crate::fixtures::{PrArgs, SubcommandSources, World};
use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use ortho_config::SubcmdConfigMerge;
use rstest_bdd_macros::{given, then, when};

fn has_no_config_sources(world: &World) -> bool {
    world
        .sub_sources
        .with_ref(SubcommandSources::is_empty)
        .unwrap_or(true)
}

fn take_sources(world: &World) -> SubcommandSources {
    world
        .sub_sources
        .take()
        .unwrap_or_default()
}

#[given("a CLI reference {reference}")]
fn set_cli_ref(world: &World, reference: String) -> Result<()> {
    ensure!(
        !reference.trim().is_empty(),
        "CLI reference must not be empty"
    );
    let mut sources = world
        .sub_sources
        .get_or_insert_with(SubcommandSources::default);
    ensure!(sources.cli.is_none(), "CLI reference already initialised");
    sources.cli = Some(reference);
    Ok(())
}

#[given("no CLI reference")]
fn no_cli_ref(world: &World) -> Result<()> {
    ensure!(
        world
            .sub_sources
            .with_ref(|sources| sources.cli.is_none())
            .unwrap_or(true),
        "CLI reference already initialised"
    );
    Ok(())
}

#[given("a configuration reference {reference}")]
fn file_ref(world: &World, reference: String) -> Result<()> {
    ensure!(
        !reference.trim().is_empty(),
        "configuration file reference must not be empty"
    );
    let mut sources = world
        .sub_sources
        .get_or_insert_with(SubcommandSources::default);
    ensure!(
        sources.file.is_none(),
        "configuration file reference already initialised"
    );
    sources.file = Some(reference);
    Ok(())
}

#[given("an environment reference {reference}")]
fn env_ref(world: &World, reference: String) -> Result<()> {
    ensure!(
        !reference.trim().is_empty(),
        "environment reference must not be empty"
    );
    let mut sources = world
        .sub_sources
        .get_or_insert_with(SubcommandSources::default);
    ensure!(
        sources.env.is_none(),
        "environment reference already initialised"
    );
    sources.env = Some(reference);
    Ok(())
}

#[when("the subcommand configuration is loaded without defaults")]
fn load_sub(world: &World) -> Result<()> {
    let sources = take_sources(world);
    let result = if sources.is_empty() {
        PrArgs::try_parse_from(["test"]).map_err(anyhow::Error::from)
    } else {
        let cli = PrArgs {
            reference: sources.cli.clone(),
        };
        setup_test_environment(&sources, &cli)?
    };
    world.sub_result.set(result);
    Ok(())
}

fn setup_test_environment(
    sources: &SubcommandSources,
    cli: &PrArgs,
) -> Result<Result<PrArgs, anyhow::Error>> {
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(file_reference) = sources.file.as_ref() {
            j.create_file(
                ".app.toml",
                &format!("[cmds.test]\nreference = \"{file_reference}\""),
            )?;
        }
        if let Some(env_reference) = sources.env.as_ref() {
            j.set_env("APP_CMDS_TEST_REFERENCE", env_reference);
        }
        result = Some(cli.load_and_merge().map_err(anyhow::Error::from));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    result.ok_or_else(|| anyhow!("subcommand merge did not run within figment jail"))
}

#[then("the merged reference is {expected}")]
fn check_ref(world: &World, expected: String) -> Result<()> {
    let result = world
        .sub_result
        .take()
        .ok_or_else(|| anyhow!("subcommand result unavailable"))?;
    let cfg = result?;
    ensure!(
        cfg.reference.as_deref() == Some(expected.as_str()),
        "unexpected reference {:?}; expected {:?}",
        cfg.reference,
        expected
    );
    Ok(())
}

#[then("the subcommand load fails")]
fn sub_error(world: &World) -> Result<()> {
    let result = world
        .sub_result
        .take()
        .ok_or_else(|| anyhow!("subcommand result unavailable"))?;
    ensure!(result.is_err(), "expected subcommand load to fail");
    Ok(())
}
