//! Steps for scenarios involving flattened CLI structures.

use crate::fixtures::{FlatArgs, FlattenContext};
use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use figment::{providers::Serialized, Figment};
use ortho_config::{
    load_config_file, sanitized_provider, OrthoError, OrthoMergeExt, OrthoResult, ResultIntoFigment,
};
use rstest_bdd_macros::{given, then, when};
use std::path::Path;

fn load_flat(file: Option<String>, args: &[&str]) -> Result<OrthoResult<FlatArgs>> {
    let mut res = None;
    figment::Jail::try_with(|j| {
        if let Some(contents) = file.as_ref() {
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

/// Helper to initialise flat_file with given content.
fn set_flat_file(flatten_context: &FlattenContext, content: impl Into<String>) -> Result<()> {
    ensure!(
        flatten_context.flat_file.is_empty(),
        "flattened configuration already initialised"
    );
    flatten_context.flat_file.set(content.into());
    Ok(())
}

#[given("the flattened configuration file has value {value}")]
fn flattened_file(flatten_context: &FlattenContext, value: String) -> Result<()> {
    set_flat_file(
        flatten_context,
        format!("nested = {{ value = \"{value}\" }}"),
    )
}

#[given("a malformed flattened configuration file")]
fn malformed_flat_file(flatten_context: &FlattenContext) -> Result<()> {
    set_flat_file(flatten_context, "nested = 5")
}

#[given("a flattened configuration file with invalid value")]
fn invalid_flat_file(flatten_context: &FlattenContext) -> Result<()> {
    set_flat_file(flatten_context, "nested = { value = 5 }")
}

#[when("the flattened config is loaded without CLI overrides")]
fn load_without_cli(flatten_context: &FlattenContext) -> Result<()> {
    let file = flatten_context.flat_file.get();
    let result = load_flat(file, &["prog"])?;
    flatten_context.flat_result.set(result);
    Ok(())
}

#[when("the flattened config is loaded with CLI value {value}")]
fn load_with_cli(flatten_context: &FlattenContext, value: String) -> Result<()> {
    let file = flatten_context.flat_file.get();
    let result = load_flat(file, &["prog", "--value", &value])?;
    flatten_context.flat_result.set(result);
    Ok(())
}

#[then("the flattened value is {expected}")]
fn check_flattened(flatten_context: &FlattenContext, expected: String) -> Result<()> {
    let result = flatten_context
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
        nested == expected.as_str(),
        "unexpected flattened value {nested:?}; expected {expected:?}"
    );
    Ok(())
}

#[then("flattening fails with a merge error")]
fn flattening_fails(flatten_context: &FlattenContext) -> Result<()> {
    let result = flatten_context
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
