//! Steps for documentation IR scenarios.

use crate::fixtures::{DocsConfig, DocsContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::OrthoConfigDocs;
use rstest_bdd_macros::{then, when};

#[when("I request the docs metadata")]
fn request_metadata(docs_context: &DocsContext) {
    let metadata = DocsConfig::get_doc_metadata();
    docs_context.metadata.set(metadata);
}

#[then("the IR version is {expected}")]
fn ir_version(docs_context: &DocsContext, expected: String) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.ir_version.clone())
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;
    ensure!(actual == expected, "expected IR version {expected}, got {actual}");
    Ok(())
}

#[then("the about id is {expected}")]
fn about_id(docs_context: &DocsContext, expected: String) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.about_id.clone())
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;
    ensure!(actual == expected, "expected about id {expected}, got {actual}");
    Ok(())
}

#[then("the help id for field {field} is {expected}")]
fn help_id_for_field(
    docs_context: &DocsContext,
    field: String,
    expected: String,
) -> Result<()> {
    let actual = field_value(docs_context, &field, |meta| meta.help_id.clone())?;
    ensure!(actual == expected, "expected help id {expected}, got {actual}");
    Ok(())
}

#[then("the long help id for field {field} is {expected}")]
fn long_help_id_for_field(
    docs_context: &DocsContext,
    field: String,
    expected: String,
) -> Result<()> {
    let actual = field_value(docs_context, &field, |meta| {
        meta.long_help_id.clone().unwrap_or_default()
    })?;
    ensure!(
        actual == expected,
        "expected long help id {expected}, got {actual}"
    );
    Ok(())
}

#[then("the environment variable for field {field} is {expected}")]
fn env_var_for_field(
    docs_context: &DocsContext,
    field: String,
    expected: String,
) -> Result<()> {
    let actual = field_value(docs_context, &field, |meta| {
        meta.env
            .as_ref()
            .map(|env| env.var_name.clone())
            .unwrap_or_default()
    })?;
    ensure!(actual == expected, "expected env var {expected}, got {actual}");
    Ok(())
}

#[then("the windows module name is {expected}")]
fn windows_module_name(docs_context: &DocsContext, expected: String) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.windows.as_ref().and_then(|meta| meta.module_name.clone()))
        .ok_or_else(|| anyhow!("docs metadata not captured"))?
        .ok_or_else(|| anyhow!("windows metadata not present"))?;
    ensure!(actual == expected, "expected module name {expected}, got {actual}");
    Ok(())
}

#[then("the windows metadata includes common parameters")]
fn windows_common_parameters(docs_context: &DocsContext) -> Result<()> {
    let include_common_parameters = docs_context
        .metadata
        .with_ref(|meta| meta.windows.as_ref().map(|meta| meta.include_common_parameters))
        .ok_or_else(|| anyhow!("docs metadata not captured"))?
        .ok_or_else(|| anyhow!("windows metadata not present"))?;
    ensure!(include_common_parameters, "expected common parameters to be included");
    Ok(())
}

#[then("the windows metadata does not split subcommands")]
fn windows_no_split(docs_context: &DocsContext) -> Result<()> {
    let split = docs_context
        .metadata
        .with_ref(|meta| meta.windows.as_ref().map(|meta| meta.split_subcommands_into_functions))
        .ok_or_else(|| anyhow!("docs metadata not captured"))?
        .ok_or_else(|| anyhow!("windows metadata not present"))?;
    ensure!(!split, "expected split_subcommands_into_functions to be false");
    Ok(())
}

fn field_value<T>(
    docs_context: &DocsContext,
    field: &str,
    f: impl FnOnce(&ortho_config::docs::FieldMetadata) -> T,
) -> Result<T> {
    let value = docs_context
        .metadata
        .with_ref(|meta| meta.fields.iter().find(|item| item.name == field).map(f))
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;
    value.ok_or_else(|| anyhow!("field {field} not found"))
}
