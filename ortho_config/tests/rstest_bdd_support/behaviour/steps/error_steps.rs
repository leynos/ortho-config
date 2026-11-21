//! Steps verifying aggregated error reporting.

use super::common::{
    SlotTakeOrExt, config_fixture, set_nonblank_scalar_once, set_scalar_once, shared_sources,
};
use super::value_parsing::{is_cli_parsing_error, normalize_scalar};
use crate::rstest_bdd_support::scenario_state::{ErrorConfig, ErrorContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig};
use rstest_bdd_macros::{given, then, when};

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
    // The guard stays bound until the loader below has read the fixture.
    let (_fixture_dir, config_path) =
        config_fixture(".ddlint.toml", Some(file_val.as_deref().unwrap_or("")))?;
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", config_path);
    if let Some(value) = env_val.as_ref() {
        source = source.with_var("DDLINT_PORT", value);
    } else if file_val.is_none() {
        // Keep this scenario focused on CLI parsing when no other source is under test.
        source = source.with_var("DDLINT_PORT", "8080");
    }
    let (discovery, merge) = shared_sources(source);
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

#[given("an automatic configuration file setting DDLINT_PORT to {value}")]
fn automatic_port(error_context: &ErrorContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_nonblank_scalar_once(
        &error_context.auto_contents,
        format!("port = {value}"),
        "automatic configuration file",
    )
}

#[when("the config is loaded with a missing selected file")]
fn load_with_missing_selected(error_context: &ErrorContext) -> Result<()> {
    let contents = error_context
        .auto_contents
        .take_or("automatic configuration file not provided")?;
    // The guard stays bound until the loader below has read the fixture.
    let (fixture_dir, _config_path) = config_fixture(".ddlint.toml", Some(&contents))?;
    let missing = fixture_dir.path().join("missing.toml");
    let source = MapEnv::new()
        .with_current_dir(fixture_dir.path())
        .with_var("HOME", fixture_dir.path().join("no-home"))
        .with_var("XDG_CONFIG_HOME", fixture_dir.path().join("no-xdg-home"))
        .with_var("XDG_CONFIG_DIRS", fixture_dir.path().join("no-xdg-dirs"))
        .with_var("DDLINT_CONFIG_PATH", missing);
    let (discovery, merge) = shared_sources(source);
    let config_result = ErrorConfig::load_from_iter_with_sources(["prog"], discovery, merge);
    error_context.agg_result.set(config_result);
    Ok(())
}

/// Returns true when `error` is, or contains, a file error.
fn is_file_error(error: &ortho_config::OrthoError) -> bool {
    match error {
        ortho_config::OrthoError::File { .. } => true,
        ortho_config::OrthoError::Aggregate(aggregated) => aggregated.iter().any(is_file_error),
        _ => false,
    }
}

#[then("a file error is returned")]
fn file_error_only(error_context: &ErrorContext) -> Result<()> {
    let result = error_context
        .agg_result
        .take_or("aggregated result unavailable")?;
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected a file error"))?;
    ensure!(is_file_error(err.as_ref()), "unexpected error: {err:?}");
    Ok(())
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
