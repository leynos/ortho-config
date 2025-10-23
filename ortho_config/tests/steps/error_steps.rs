//! Steps verifying aggregated error reporting.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{ErrorConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[given("an invalid configuration file")]
fn invalid_file(world: &mut World) -> Result<()> {
    ensure!(
        world.file_value.is_none(),
        "invalid configuration file already initialised"
    );
    world.file_value = Some("port = ".into());
    Ok(())
}

#[given(expr = "the environment variable DDLINT_PORT is {string}")]
fn env_port(world: &mut World, val: String) -> Result<()> {
    ensure!(
        !val.trim().is_empty(),
        "environment port value must not be empty"
    );
    ensure!(
        world.env_value.is_none(),
        "environment port already initialised"
    );
    world.env_value = Some(val);
    Ok(())
}

#[when("the config is loaded with an invalid CLI argument")]
fn load_invalid_cli(world: &mut World) -> Result<()> {
    let file_val = world.file_value.clone();
    let env_val = world.env_value.clone();
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(value) = file_val.as_ref() {
            j.create_file(".ddlint.toml", value)?;
        }
        if let Some(value) = env_val.as_ref() {
            j.set_env("DDLINT_PORT", value);
        }
        result = Some(ErrorConfig::load_from_iter(["prog", "--bogus"]));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    world.agg_result = result;
    ensure!(
        world.agg_result.is_some(),
        "error aggregation load did not produce a result"
    );
    Ok(())
}

#[then("CLI, file and environment errors are returned")]
fn cli_file_env_errors(world: &mut World) -> Result<()> {
    let result = world
        .agg_result
        .take()
        .ok_or_else(|| anyhow!("aggregated result unavailable"))?;
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected aggregated error"))?;
    match err.as_ref() {
        ortho_config::OrthoError::Aggregate(agg) => {
            let mut saw_cli = false;
            let mut saw_file = false;
            let mut saw_env = false;
            for entry in agg.iter() {
                match entry {
                    ortho_config::OrthoError::CliParsing(_) => saw_cli = true,
                    ortho_config::OrthoError::File { .. } => saw_file = true,
                    ortho_config::OrthoError::Merge { .. }
                    | ortho_config::OrthoError::Gathering(_) => saw_env = true,
                    _ => {}
                }
            }
            ensure!(saw_cli, "expected CLI parsing error in aggregate");
            ensure!(saw_file, "expected file error in aggregate");
            ensure!(saw_env, "expected environment error in aggregate");
            Ok(())
        }
        other => Err(anyhow!("unexpected error: {other:?}")),
    }
}

#[then("a CLI parsing error is returned")]
fn cli_error_only(world: &mut World) -> Result<()> {
    let result = world
        .agg_result
        .take()
        .ok_or_else(|| anyhow!("aggregated result unavailable"))?;
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected CLI parsing error"))?;
    match err.as_ref() {
        ortho_config::OrthoError::CliParsing(_) => Ok(()),
        other => Err(anyhow!("unexpected error: {other:?}")),
    }
}
