//! Helpers for documentation IR step assertions.

use std::ops::Deref;

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::FieldMetadata;

use crate::fixtures::DocsContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldName(String);

impl FieldName {
    pub(crate) fn new(value: String) -> Self { Self(value) }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl Deref for FieldName {
    type Target = str;

    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl From<String> for FieldName {
    fn from(value: String) -> Self { Self(value) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpectedId(String);

impl ExpectedId {
    pub(crate) fn new(value: String) -> Self { Self(value) }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl Deref for ExpectedId {
    type Target = str;

    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl From<String> for ExpectedId {
    fn from(value: String) -> Self { Self(value) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpectedValue(String);

impl ExpectedValue {
    pub(crate) fn new(value: String) -> Self { Self(value) }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl Deref for ExpectedValue {
    type Target = str;

    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl From<String> for ExpectedValue {
    fn from(value: String) -> Self { Self(value) }
}

pub(crate) fn assert_ir_version(
    docs_context: &DocsContext,
    expected: &ExpectedValue,
) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.ir_version.clone())
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;
    ensure!(
        actual == expected.as_str(),
        "expected IR version {}, got {actual}",
        expected.as_str()
    );
    Ok(())
}

pub(crate) fn assert_about_id(docs_context: &DocsContext, expected: &ExpectedId) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.about_id.clone())
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;
    ensure!(
        actual == expected.as_str(),
        "expected about id {}, got {actual}",
        expected.as_str()
    );
    Ok(())
}

pub(crate) fn assert_field_help_id(
    docs_context: &DocsContext,
    field: &FieldName,
    expected: &ExpectedId,
) -> Result<()> {
    let actual = field_value(docs_context, field, |meta| meta.help_id.clone())?;
    ensure!(
        actual == expected.as_str(),
        "expected help id {}, got {actual}",
        expected.as_str()
    );
    Ok(())
}

pub(crate) fn assert_field_long_help_id(
    docs_context: &DocsContext,
    field: &FieldName,
    expected: &ExpectedId,
) -> Result<()> {
    let actual = field_value(docs_context, field, |meta| {
        meta.long_help_id.clone().unwrap_or_default()
    })?;
    ensure!(
        actual == expected.as_str(),
        "expected long help id {}, got {actual}",
        expected.as_str()
    );
    Ok(())
}

pub(crate) fn assert_field_env_var(
    docs_context: &DocsContext,
    field: &FieldName,
    expected: &ExpectedValue,
) -> Result<()> {
    let actual = field_value(docs_context, field, |meta| {
        meta.env
            .as_ref()
            .map(|env| env.var_name.clone())
            .unwrap_or_default()
    })?;
    ensure!(
        actual == expected.as_str(),
        "expected env var {}, got {actual}",
        expected.as_str()
    );
    Ok(())
}

pub(crate) fn assert_windows_module_name(
    docs_context: &DocsContext,
    expected: &ExpectedValue,
) -> Result<()> {
    let actual = docs_context
        .metadata
        .with_ref(|meta| meta.windows.as_ref().and_then(|meta| meta.module_name.clone()))
        .ok_or_else(|| anyhow!("docs metadata not captured"))?
        .ok_or_else(|| anyhow!("windows metadata not present"))?;
    ensure!(
        actual == expected.as_str(),
        "expected module name {}, got {actual}",
        expected.as_str()
    );
    Ok(())
}

pub(crate) fn field_value<T>(
    docs_context: &DocsContext,
    field: &FieldName,
    f: impl FnOnce(&FieldMetadata) -> T,
) -> Result<T> {
    let value = docs_context
        .metadata
        .with_ref(|meta| meta.fields.iter().find(|item| item.name == field.as_str()).map(f))
        .ok_or_else(|| anyhow!("docs metadata not captured"))?;
    value.ok_or_else(|| anyhow!("field {} not found", field.as_str()))
}
