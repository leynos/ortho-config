//! Steps verifying aggregated error reporting.

use super::common::{SlotTakeOrExt, set_nonblank_scalar_once, set_scalar_once};
use super::value_parsing::{is_cli_parsing_error, normalize_scalar};
use crate::scenario_state::{ErrorConfig, ErrorContext};
use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{given, then, when};
use std::sync::Arc;

#[given("an invalid configuration file")]
fn invalid_file(error_context: &ErrorContext) -> Result<()> {
    set_scalar_once(
        &error_context.file_value,
        "port = ",
        "invalid configuration file",
    )
}

#[given("the environment variable DDLINT_PORT is {value}")]
fn env_port(error_context: &ErrorContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_nonblank_scalar_once(&error_context.env_value, value, "environment port value")
}

#[when("the config is loaded with an invalid CLI argument")]
fn load_invalid_cli(error_context: &ErrorContext) -> Result<()> {
    let file_val = error_context.file_value.get();
    let env_val = error_context.env_value.get();
    let fixture_dir = tempfile::tempdir().context("create error config fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open error config fixture directory")?;
    fixture
        .write(".ddlint.toml", file_val.as_deref().unwrap_or("").as_bytes())
        .context("write error config fixture")?;
    let config_path = fixture_dir.path().join(".ddlint.toml");
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", config_path);
    if let Some(value) = env_val.as_ref() {
        source = source.with_var("DDLINT_PORT", value);
    } else if file_val.is_none() {
        // Keep this scenario focused on CLI parsing when no other source is under test.
        source = source.with_var("DDLINT_PORT", "8080");
    }
    let source = Arc::new(source);
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let config_result =
        ErrorConfig::load_from_iter_with_sources(["prog", "--bogus"], discovery, merge);
    error_context.agg_result.set(config_result);
    Ok(())
}

#[then("CLI, file and environment errors are returned")]
fn cli_file_env_errors(error_context: &ErrorContext) -> Result<()> {
    let result = error_context
        .agg_result
        .take_or("aggregated result unavailable")?;
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
        .take_or("aggregated result unavailable")?;
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected CLI parsing error"))?;
    if is_cli_parsing_error(err.as_ref()) {
        Ok(())
    } else {
        Err(anyhow!("unexpected error: {err:?}"))
    }
}
