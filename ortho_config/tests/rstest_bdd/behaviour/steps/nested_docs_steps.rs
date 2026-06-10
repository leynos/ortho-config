//! Steps for nested documentation IR scenarios.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, FieldMetadata, OrthoConfigDocs};
use rstest_bdd_macros::{given, then, when};

use crate::scenario_state::{NestedDocsConfig, NestedDocsContext};

use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandPath(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuotedName(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandNameList(pub Vec<String>);

impl FromStr for CommandPath {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').replace("\\\"", "\"")))
    }
}

impl FromStr for QuotedName {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').replace("\\\"", "\"")))
    }
}

impl FromStr for CommandNameList {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let names = s
            .split(',')
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect();
        Ok(Self(names))
    }
}

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
    expected: CommandNameList,
) -> Result<()> {
    let actual = nested_docs_context
        .metadata
        .with_ref(command_names)
        .ok_or_else(|| anyhow!("nested docs metadata not captured"))?;
    let expected = expected.0;

    ensure!(
        actual == expected,
        "expected nested top-level commands {expected:?}, got {actual:?}",
    );
    Ok(())
}

fn in_command(
    context: &NestedDocsContext,
    command: CommandPath,
    f: impl FnOnce(&DocMetadata, &str) -> Result<()>,
) -> Result<()> {
    with_command(context, &command.0, |meta| f(meta, &command.0))
}

#[then("command {command} contains field {field}")]
fn command_contains_field(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
    field: QuotedName,
) -> Result<()> {
    let field_name = field.0;
    in_command(nested_docs_context, command, |meta, _cmd| {
        field_by_name(meta, &field_name).map(|_| ())
    })
}

#[then("command {command} field {field} has default {expected}")]
fn command_field_has_default(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
    field: QuotedName,
    expected: QuotedName,
) -> Result<()> {
    in_command(nested_docs_context, command, |meta, _cmd| {
        let field_name = field.0.clone();
        let expected = expected.0.clone();
        let field = field_by_name(meta, &field_name)?;
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
    command: CommandPath,
    expected: QuotedName,
) -> Result<()> {
    let expected = expected.0;
    in_command(nested_docs_context, command, |meta, cmd| {
        ensure!(
            meta.sections
                .examples
                .iter()
                .any(|example| example.code == expected),
            "expected command {cmd:?} to include example {expected:?}",
        );
        Ok(())
    })
}

#[then("command {command} exposes no fields")]
fn command_exposes_no_fields(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
) -> Result<()> {
    in_command(nested_docs_context, command, |meta, cmd| {
        ensure!(
            meta.fields.is_empty(),
            "expected command {cmd:?} to expose no fields, got {:?}",
            field_names(meta),
        );
        Ok(())
    })
}

#[then("command {command} contains nested commands {expected}")]
fn command_contains_nested_commands(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
    expected: CommandNameList,
) -> Result<()> {
    in_command(nested_docs_context, command, |meta, cmd| {
        let actual = command_names(meta);
        let expected = expected.0.clone();
        ensure!(
            actual == expected,
            "expected command {cmd:?} to expose {expected:?}, got {actual:?}",
        );
        Ok(())
    })
}

#[then("command {command} exposes Windows wrapper metadata")]
fn command_exposes_windows_metadata(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
) -> Result<()> {
    in_command(nested_docs_context, command, |meta, cmd| {
        ensure!(
            meta.windows.is_some(),
            "expected command {cmd:?} to expose Windows metadata",
        );
        Ok(())
    })
}

#[then("command {command} splits subcommands into functions")]
fn command_splits_subcommands(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
) -> Result<()> {
    in_command(nested_docs_context, command, |meta, cmd| {
        let split = meta
            .windows
            .as_ref()
            .ok_or_else(|| anyhow!("missing Windows metadata for command {cmd}"))?
            .split_subcommands_into_functions;
        ensure!(
            split,
            "expected command {cmd:?} to split subcommands into functions",
        );
        Ok(())
    })
}

#[then("command {command} exposes no Windows wrapper metadata")]
fn command_exposes_no_windows_metadata(
    nested_docs_context: &NestedDocsContext,
    command: CommandPath,
) -> Result<()> {
    in_command(nested_docs_context, command, |meta, cmd| {
        ensure!(
            meta.windows.is_none(),
            "expected command {cmd:?} to expose no Windows metadata",
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
