//! Steps for documentation IR scenarios.
//!
//! Note: This module has a high percentage of String parameters (50%) because
//! rstest_bdd requires step functions to accept String parameters from Gherkin
//! scenarios. The functions immediately convert these to semantic newtypes and
//! delegate to type-safe helpers in the `helpers` module, ensuring type safety
//! in the actual implementation layer.

mod helpers;

use crate::fixtures::{DocsConfig, DocsContext};
use self::helpers::{
    ExpectedId, ExpectedValue, FieldName, assert_about_id, assert_field_env_var,
    assert_field_help_id, assert_field_long_help_id, assert_ir_version,
    assert_windows_module_name, windows_value,
};
use anyhow::{Result, ensure};
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
fn help_id_for_field(
    docs_context: &DocsContext,
    field: String,
    expected: String,
) -> Result<()> {
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
fn env_var_for_field(
    docs_context: &DocsContext,
    field: String,
    expected: String,
) -> Result<()> {
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
    ensure!(include_common_parameters, "expected common parameters to be included");
    Ok(())
}

#[then("the windows metadata does not split subcommands")]
fn windows_no_split(docs_context: &DocsContext) -> Result<()> {
    let split = windows_value(docs_context, |w| w.split_subcommands_into_functions)?;
    ensure!(!split, "expected split_subcommands_into_functions to be false");
    Ok(())
}
