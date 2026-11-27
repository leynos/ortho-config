//! Proves documented environment names follow Serde's deserialization keys.
//!
//! The loader deserializes the environment and file layers, so a field's real
//! binding is Serde's deserialization key, not the Rust identifier and not a
//! serialization-only rename. This suite binds the metadata and the loader
//! together for both directions of `#[serde(rename(...))]`.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{FieldMetadata, OrthoConfigDocs as _};
use ortho_config::{MapEnv, OrthoConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A configuration whose fields bind under names other than their identifiers.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "RENAME_APP_")]
struct RenamedConfig {
    /// Check command, stored under its declared input key.
    #[serde(rename = "verify")]
    check: Option<String>,
    /// Serialization-only rename: the input key stays `report`.
    #[serde(rename(serialize = "report-out"))]
    report: Option<String>,
    /// Deserialization-only rename: the input key becomes `incoming`.
    #[serde(rename(deserialize = "incoming"))]
    arrival: Option<String>,
}

/// Returns one documented field by name.
fn field<'a>(
    metadata: &'a ortho_config::docs::DocMetadata,
    name: &str,
) -> Result<&'a FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| anyhow!("missing {name} field metadata"))
}

/// Asserts the documented environment name and file key for one field.
fn ensure_bindings(
    metadata: &ortho_config::docs::DocMetadata,
    name: &str,
    env_name: &str,
    file_key: &str,
) -> Result<()> {
    let field = field(metadata, name)?;
    ensure!(
        field.env.as_ref().map(|env| env.var_name.as_str()) == Some(env_name),
        "{name} should document environment name {env_name}, got {:?}",
        field.env
    );
    ensure!(
        field.file.as_ref().map(|file| file.key_path.as_str()) == Some(file_key),
        "{name} should document file key {file_key}, got {:?}",
        field.file
    );
    Ok(())
}

#[test]
fn documented_bindings_follow_the_deserialization_key() -> Result<()> {
    let metadata = RenamedConfig::get_doc_metadata();

    ensure_bindings(&metadata, "check", "RENAME_APP_VERIFY", "verify")?;
    ensure_bindings(&metadata, "report", "RENAME_APP_REPORT", "report")?;
    ensure_bindings(&metadata, "arrival", "RENAME_APP_INCOMING", "incoming")?;
    Ok(())
}

#[test]
fn every_documented_environment_name_loads_its_field() -> Result<()> {
    let env = MapEnv::new()
        .with_var("RENAME_APP_VERIFY", "checked")
        .with_var("RENAME_APP_REPORT", "reported")
        .with_var("RENAME_APP_INCOMING", "arrived");
    let source = Arc::new(env);
    let config = RenamedConfig::load_from_iter_with_sources(["app"], source.clone(), source)
        .map_err(|err| anyhow!(err))?;

    ensure!(
        config.check.as_deref() == Some("checked"),
        "the documented env name should load the renamed field"
    );
    ensure!(
        config.report.as_deref() == Some("reported"),
        "a serialization-only rename must not move the input name"
    );
    ensure!(
        config.arrival.as_deref() == Some("arrived"),
        "a deserialization-only rename must move the input name"
    );
    Ok(())
}
