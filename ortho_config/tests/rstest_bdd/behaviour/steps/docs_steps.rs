//! Steps for documentation IR scenarios.
//!
//! Note: This module has a high percentage of String parameters (50%) because
//! rstest_bdd requires step functions to accept String parameters from Gherkin
//! scenarios. The functions immediately convert these to semantic newtypes and
//! delegate to type-safe helpers in the `helpers` module, ensuring type safety
//! in the actual implementation layer.

#[path = "helpers.rs"]
mod helpers;

use self::helpers::{
    ExpectedId, ExpectedValue, FieldName, assert_about_id, assert_field_env_var,
    assert_field_help_id, assert_field_long_help_id, assert_ir_version, assert_windows_module_name,
    windows_value,
};
use crate::scenario_state::{DocsConfig, DocsContext};
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
    let expected = ExpectedValue::from(expected);
    assert_ir_version(docs_context, &expected)
}

#[then("the about id is {expected}")]
fn about_id(docs_context: &DocsContext, expected: String) -> Result<()> {
    let expected = ExpectedId::from(expected);
    assert_about_id(docs_context, &expected)
}

#[then("the help id for field {field} is {expected}")]
fn help_id_for_field(docs_context: &DocsContext, field: String, expected: String) -> Result<()> {
    let field = FieldName::from(field);
    let expected = ExpectedId::from(expected);
    assert_field_help_id(docs_context, &field, &expected)
}

#[then("the long help id for field {field} is {expected}")]
fn long_help_id_for_field(
    docs_context: &DocsContext,
    field: String,
    expected: String,
) -> Result<()> {
    let field = FieldName::from(field);
    let expected = ExpectedId::from(expected);
    assert_field_long_help_id(docs_context, &field, &expected)
}

#[then("the environment variable for field {field} is {expected}")]
fn env_var_for_field(docs_context: &DocsContext, field: String, expected: String) -> Result<()> {
    let field = FieldName::from(field);
    let expected = ExpectedValue::from(expected);
    assert_field_env_var(docs_context, &field, &expected)
}

#[then("the windows module name is {expected}")]
fn windows_module_name(docs_context: &DocsContext, expected: String) -> Result<()> {
    let expected = ExpectedValue::from(expected);
    assert_windows_module_name(docs_context, &expected)
}

#[then("the windows metadata includes common parameters")]
fn windows_common_parameters(docs_context: &DocsContext) -> Result<()> {
    let include_common_parameters = windows_value(docs_context, |w| w.include_common_parameters)?;
    ensure!(
        include_common_parameters,
        "expected common parameters to be included"
    );
    Ok(())
}

#[then("the windows metadata does not split subcommands")]
fn windows_no_split(docs_context: &DocsContext) -> Result<()> {
    let split = windows_value(docs_context, |w| w.split_subcommands_into_functions)?;
    ensure!(
        !split,
        "expected split_subcommands_into_functions to be false"
    );
    Ok(())
}

#[then("the subcommands are {expected}")]
fn subcommands_are(docs_context: &DocsContext, expected: String) -> Result<()> {
    let expected = expected
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let actual = docs_context
        .metadata
        .with_ref(|meta| {
            meta.subcommands
                .iter()
                .map(|subcommand| subcommand.app_name.clone())
                .collect::<Vec<_>>()
        })
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;

    ensure!(
        actual == expected,
        "expected subcommands {expected:?}, got {actual:?}"
    );
    Ok(())
}

#[then("subcommand {name} has app name {expected}")]
fn subcommand_has_app_name(
    docs_context: &DocsContext,
    name: String,
    expected: String,
) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| {
            meta.subcommands
                .iter()
                .find(|subcommand| subcommand.app_name == name)
                .map(|subcommand| subcommand.app_name.clone())
        })
        .ok_or_else(|| anyhow!("docs metadata not captured"))?
        .ok_or_else(|| anyhow!("subcommand {name} not found"))?;

    ensure!(
        actual == expected,
        "expected app name {expected}, got {actual}"
    );
    Ok(())
}

#[then("the commands heading id is {expected}")]
fn commands_heading_id(docs_context: &DocsContext, expected: String) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.sections.headings_ids.commands.clone())
        .ok_or_else(|| anyhow!("docs metadata not captured"))?
        .ok_or_else(|| anyhow!("commands heading id not present"))?;

    ensure!(
        actual == expected,
        "expected commands heading id {expected}, got {actual}"
    );
    Ok(())
}
