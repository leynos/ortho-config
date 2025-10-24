//! Steps for scenarios involving flattened CLI structures.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]
use crate::{FlatArgs, World};
use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use cucumber::{given, then, when};
use figment::{Figment, providers::Serialized};
use ortho_config::{
    OrthoError, OrthoMergeExt, OrthoResult, ResultIntoFigment, load_config_file, sanitized_provider,
};
use std::path::Path;

fn load_flat(file: Option<&str>, args: &[&str]) -> Result<OrthoResult<FlatArgs>> {
    let mut res = None;
    figment::Jail::try_with(|j| {
        if let Some(contents) = file {
            j.create_file(".flat.toml", contents)?;
        }
        let cli = FlatArgs::parse_from(args);
        let mut fig = Figment::from(Serialized::defaults(&FlatArgs::default()));
        if let Some(f) = load_config_file(Path::new(".flat.toml")).to_figment()? {
            fig = fig.merge(f);
        }
        res = Some(
            fig.merge(sanitized_provider(&cli).to_figment()?)
                .extract()
                .into_ortho_merge(),
        );
        Ok(())
    })
    .map_err(anyhow::Error::new)?;
    res.ok_or_else(|| anyhow!("flattened configuration load did not produce a result"))
}

#[given(expr = "the flattened configuration file has value {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn flattened_file(world: &mut World, val: String) -> Result<()> {
    ensure!(
        world.flat_file.is_none(),
        "flattened configuration already initialised"
    );
    world.flat_file = Some(format!("nested = {{ value = \"{val}\" }}"));
    Ok(())
}

#[given("a malformed flattened configuration file")]
fn malformed_flat_file(world: &mut World) -> Result<()> {
    ensure!(
        world.flat_file.is_none(),
        "flattened configuration already initialised"
    );
    world.flat_file = Some("nested = 5".into());
    Ok(())
}

#[given("a flattened configuration file with invalid value")]
fn invalid_flat_file(world: &mut World) -> Result<()> {
    ensure!(
        world.flat_file.is_none(),
        "flattened configuration already initialised"
    );
    world.flat_file = Some("nested = { value = 5 }".into());
    Ok(())
}

#[when("the flattened config is loaded without CLI overrides")]
fn load_without_cli(world: &mut World) -> Result<()> {
    world.flat_result = Some(load_flat(world.flat_file.as_deref(), &["prog"])?);
    Ok(())
}

#[when(expr = "the flattened config is loaded with CLI value {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn load_with_cli(world: &mut World, cli: String) -> Result<()> {
    world.flat_result = Some(load_flat(
        world.flat_file.as_deref(),
        &["prog", "--value", &cli],
    )?);
    Ok(())
}

#[then(expr = "the flattened value is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn check_flattened(world: &mut World, expected_value: String) -> Result<()> {
    let result = world
        .flat_result
        .take()
        .ok_or_else(|| anyhow!("flattened configuration result unavailable"))?;
    let cfg = result?;
    let nested = cfg
        .nested
        .value
        .as_deref()
        .ok_or_else(|| anyhow!("expected nested value to be present"))?;
    ensure!(
        nested == expected_value.as_str(),
        "unexpected flattened value {nested:?}; expected {:?}",
        expected_value
    );
    Ok(())
}

#[then("flattening fails with a merge error")]
fn flattening_fails(world: &mut World) -> Result<()> {
    let result = world
        .flat_result
        .take()
        .ok_or_else(|| anyhow!("flattened configuration result unavailable"))?;
    match result {
        Ok(_) => Err(anyhow!("expected merge error but configuration succeeded")),
        Err(err) => {
            ensure!(
                matches!(err.as_ref(), OrthoError::Merge { .. }),
                "unexpected error: {err:?}"
            );
            Ok(())
        }
    }
}
