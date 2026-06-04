//! Steps for nested documentation IR scenarios.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, FieldMetadata, OrthoConfigDocs};
use rstest_bdd_macros::{given, then, when};

use crate::scenario_state::{NestedDocsConfig, NestedDocsContext};

#[given("the nested CLI fixture")]
fn nested_cli_fixture(nested_docs_context: &NestedDocsContext) {
    let _ = nested_docs_context;
}

#[when("I request the nested docs metadata")]
fn request_nested_metadata(nested_docs_context: &NestedDocsContext) {
    nested_docs_context
        .metadata
        .set(NestedDocsConfig::get_doc_metadata());
}

#[then("the nested top-level commands are {expected}")]
fn nested_top_level_commands(
    nested_docs_context: &NestedDocsContext,
    expected: String,
) -> Result<()> {
    let actual = nested_docs_context
        .metadata
        .with_ref(command_names)
        .ok_or_else(|| anyhow!("nested docs metadata not captured"))?;
    let expected = names_from_csv(&expected);

    ensure!(
        actual == expected,
        "expected nested top-level commands {expected:?}, got {actual:?}",
    );
    Ok(())
}

#[then("command {command} contains field {field}")]
fn command_contains_field(
    nested_docs_context: &NestedDocsContext,
    command: String,
    field: String,
) -> Result<()> {
    let command = unquoted(&command);
    let field = unquoted(&field);
    with_command(nested_docs_context, &command, |metadata| {
        field_by_name(metadata, &field).map(|_| ())
    })
}

#[then("command {command} field {field} has default {expected}")]
fn command_field_has_default(
    nested_docs_context: &NestedDocsContext,
    command: String,
    field: String,
    expected: String,
) -> Result<()> {
    let command = unquoted(&command);
    let field_name = unquoted(&field);
    let expected = unquoted(&expected);
    with_command(nested_docs_context, &command, |metadata| {
        let field = field_by_name(metadata, &field_name)?;
        let actual = field.default.as_ref().map(|value| value.display.as_str());
        ensure!(
            actual == Some(expected.as_str()),
            "expected default {expected:?} for field {field_name:?}, got {actual:?}",
        );
        Ok(())
    })
}

#[then("command {command} has example {expected}")]
fn command_has_example(
    nested_docs_context: &NestedDocsContext,
    command: String,
    expected: String,
) -> Result<()> {
    let command = unquoted(&command);
    let expected = unquoted(&expected);
    with_command(nested_docs_context, &command, |metadata| {
        ensure!(
            metadata
                .sections
                .examples
                .iter()
                .any(|example| example.code == expected),
            "expected command {command:?} to include example {expected:?}",
        );
        Ok(())
    })
}

#[then("command {command} exposes no fields")]
fn command_exposes_no_fields(
    nested_docs_context: &NestedDocsContext,
    command: String,
) -> Result<()> {
    let command = unquoted(&command);
    with_command(nested_docs_context, &command, |metadata| {
        ensure!(
            metadata.fields.is_empty(),
            "expected command {command:?} to expose no fields, got {:?}",
            field_names(metadata),
        );
        Ok(())
    })
}

#[then("command {command} contains nested commands {expected}")]
fn command_contains_nested_commands(
    nested_docs_context: &NestedDocsContext,
    command: String,
    expected: String,
) -> Result<()> {
    let command = unquoted(&command);
    with_command(nested_docs_context, &command, |metadata| {
        let actual = command_names(metadata);
        let expected = names_from_csv(&expected);
        ensure!(
            actual == expected,
            "expected command {command:?} to expose {expected:?}, got {actual:?}",
        );
        Ok(())
    })
}

#[then("command {command} exposes Windows wrapper metadata")]
fn command_exposes_windows_metadata(
    nested_docs_context: &NestedDocsContext,
    command: String,
) -> Result<()> {
    let command = unquoted(&command);
    with_command(nested_docs_context, &command, |metadata| {
        ensure!(
            metadata.windows.is_some(),
            "expected command {command:?} to expose Windows metadata",
        );
        Ok(())
    })
}

#[then("command {command} splits subcommands into functions")]
fn command_splits_subcommands(
    nested_docs_context: &NestedDocsContext,
    command: String,
) -> Result<()> {
    let command = unquoted(&command);
    with_command(nested_docs_context, &command, |metadata| {
        let split = metadata
            .windows
            .as_ref()
            .ok_or_else(|| anyhow!("missing Windows metadata for command {command}"))?
            .split_subcommands_into_functions;
        ensure!(
            split,
            "expected command {command:?} to split subcommands into functions",
        );
        Ok(())
    })
}

#[then("command {command} exposes no Windows wrapper metadata")]
fn command_exposes_no_windows_metadata(
    nested_docs_context: &NestedDocsContext,
    command: String,
) -> Result<()> {
    let command = unquoted(&command);
    with_command(nested_docs_context, &command, |metadata| {
        ensure!(
            metadata.windows.is_none(),
            "expected command {command:?} to expose no Windows metadata",
        );
        Ok(())
    })
}

fn with_command(
    nested_docs_context: &NestedDocsContext,
    command: &str,
    assertion: impl FnOnce(&DocMetadata) -> Result<()>,
) -> Result<()> {
    nested_docs_context
        .metadata
        .with_ref(|metadata| {
            let command = command_by_path(metadata, command)?;
            assertion(command)
        })
        .ok_or_else(|| anyhow!("nested docs metadata not captured"))?
}

fn command_by_path<'metadata>(
    metadata: &'metadata DocMetadata,
    command: &str,
) -> Result<&'metadata DocMetadata> {
    command
        .split_whitespace()
        .try_fold(metadata, |current, name| {
            current
                .subcommands
                .iter()
                .find(|subcommand| subcommand.app_name == name)
                .ok_or_else(|| anyhow!("command {command:?} segment {name:?} not found"))
        })
}

fn field_by_name<'metadata>(
    metadata: &'metadata DocMetadata,
    name: &str,
) -> Result<&'metadata FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| anyhow!("field {name:?} not found"))
}

fn names_from_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn unquoted(value: &str) -> String {
    value.trim_matches('"').replace("\\\"", "\"")
}

fn command_names(metadata: &DocMetadata) -> Vec<String> {
    metadata
        .subcommands
        .iter()
        .map(|command| command.app_name.clone())
        .collect()
}

fn field_names(metadata: &DocMetadata) -> Vec<String> {
    metadata
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect()
}
