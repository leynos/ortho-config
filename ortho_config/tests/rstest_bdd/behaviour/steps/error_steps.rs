//! Steps verifying aggregated error reporting.

use crate::fixtures::{ErrorConfig, ErrorContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};

#[given("an invalid configuration file")]
fn invalid_file(error_context: &ErrorContext) -> Result<()> {
    ensure!(
        error_context.file_value.is_empty(),
        "invalid configuration file already initialised"
    );
    error_context.file_value.set("port = ".into());
    Ok(())
}

#[given("the environment variable DDLINT_PORT is {value}")]
fn env_port(error_context: &ErrorContext, value: String) -> Result<()> {
    ensure!(
        !value.trim().is_empty(),
        "environment port value must not be empty"
    );
    ensure!(
        error_context.env_value.is_empty(),
        "environment port already initialised"
    );
    error_context.env_value.set(value);
    Ok(())
}

#[when("the config is loaded with an invalid CLI argument")]
fn load_invalid_cli(error_context: &ErrorContext) -> Result<()> {
    let file_val = error_context.file_value.get();
    let env_val = error_context.env_value.get();
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
    let config_result =
        result.ok_or_else(|| anyhow!("error aggregation load did not produce a result"))?;
    error_context.agg_result.set(config_result);
    Ok(())
}

#[then("CLI, file and environment errors are returned")]
fn cli_file_env_errors(error_context: &ErrorContext) -> Result<()> {
    let result = error_context
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
fn cli_error_only(error_context: &ErrorContext) -> Result<()> {
    let result = error_context
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
