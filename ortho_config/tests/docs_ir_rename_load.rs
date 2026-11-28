//! Proves documented input names follow Serde's deserialization contract.
//!
//! The loader deserializes the environment and file layers, so a field's real
//! binding is Serde's deserialization key, not the Rust identifier, a
//! serialization-only rename, or the serialization side of a directional
//! `rename_all`. This suite binds the metadata and the loader together for both
//! directions of `rename` and `rename_all`, and proves that a generated
//! environment route reproduces the key the provider must read.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, FieldMetadata, OrthoConfigDocs as _};
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
    /// A kebab-case key keeps the hyphen in its environment spelling.
    #[serde(rename = "verify-mode")]
    verify_mode: Option<String>,
    /// A camel-case key is readable case-insensitively by the provider.
    #[serde(rename = "logLevel")]
    log_level: Option<String>,
}

/// A container whose deserialization rule is kebab-case.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "DIRECTIONAL_APP_")]
#[serde(rename_all(deserialize = "kebab-case"))]
struct DirectionalRenameAll {
    /// Key Serde reads as `output-file`.
    output_file: Option<String>,
}

/// A container whose serialization rule must not move the input name.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "SERIALIZE_APP_")]
#[serde(rename_all(serialize = "SCREAMING_SNAKE_CASE"))]
struct SerializeOnlyRenameAll {
    /// Key Serde reads as `output_file`.
    output_file: Option<String>,
}

/// Returns one documented field by name.
fn field<'a>(metadata: &'a DocMetadata, name: &str) -> Result<&'a FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| anyhow!("missing {name} field metadata"))
}

/// Asserts the documented environment route and file key for one field.
///
/// `env_name: None` requires the field to publish no environment route.
fn ensure_bindings(
    metadata: &DocMetadata,
    name: &str,
    env_name: Option<&str>,
    file_key: &str,
) -> Result<()> {
    let field = field(metadata, name)?;
    match env_name {
        Some(expected) => ensure!(
            field.env.as_ref().map(|env| env.var_name.as_str()) == Some(expected),
            "{name} should document environment name {expected}, got {:?}",
            field.env
        ),
        None => ensure!(
            field.env.is_none(),
            "{name} should publish no environment route, got {:?}",
            field.env
        ),
    }
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

    ensure_bindings(&metadata, "check", Some("RENAME_APP_VERIFY"), "verify")?;
    ensure_bindings(&metadata, "report", Some("RENAME_APP_REPORT"), "report")?;
    ensure_bindings(
        &metadata,
        "arrival",
        Some("RENAME_APP_INCOMING"),
        "incoming",
    )?;
    ensure_bindings(
        &metadata,
        "verify_mode",
        Some("RENAME_APP_VERIFY-MODE"),
        "verify-mode",
    )?;
    ensure_bindings(&metadata, "log_level", None, "logLevel")?;
    Ok(())
}

#[test]
fn directional_rename_all_follows_its_deserialization_side() -> Result<()> {
    let deserialized = DirectionalRenameAll::get_doc_metadata();
    ensure_bindings(
        &deserialized,
        "output_file",
        Some("DIRECTIONAL_APP_OUTPUT-FILE"),
        "output-file",
    )?;

    let serialized = SerializeOnlyRenameAll::get_doc_metadata();
    ensure_bindings(
        &serialized,
        "output_file",
        Some("SERIALIZE_APP_OUTPUT_FILE"),
        "output_file",
    )?;
    Ok(())
}

/// Loads a configuration from the supplied variable and returns it.
fn load_renamed(entries: &[(&str, &str)]) -> Result<RenamedConfig> {
    let mut env = MapEnv::new();
    for (name, value) in entries {
        env = env.with_var(*name, *value);
    }
    let source = Arc::new(env);
    RenamedConfig::load_from_iter_with_sources(["app"], source.clone(), source)
        .map_err(|err| anyhow!(err))
}

#[test]
fn every_documented_environment_name_loads_its_field() -> Result<()> {
    let config = load_renamed(&[
        ("RENAME_APP_VERIFY", "checked"),
        ("RENAME_APP_REPORT", "reported"),
        ("RENAME_APP_INCOMING", "arrived"),
        ("RENAME_APP_VERIFY-MODE", "strict"),
    ])?;

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
    ensure!(
        config.verify_mode.as_deref() == Some("strict"),
        "a hyphenated key must load from its documented variable"
    );
    Ok(())
}

#[test]
fn directional_rename_all_loads_from_its_documented_route() -> Result<()> {
    let deserializing_env = MapEnv::new().with_var("DIRECTIONAL_APP_OUTPUT-FILE", "out.txt");
    let deserializing_source = Arc::new(deserializing_env);
    let deserializing = DirectionalRenameAll::load_from_iter_with_sources(
        ["app"],
        deserializing_source.clone(),
        deserializing_source,
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        deserializing.output_file.as_deref() == Some("out.txt"),
        "the deserialization-side container rule must name the loader's key"
    );

    let serializing_env = MapEnv::new().with_var("SERIALIZE_APP_OUTPUT_FILE", "out.txt");
    let serializing_source = Arc::new(serializing_env);
    let serializing = SerializeOnlyRenameAll::load_from_iter_with_sources(
        ["app"],
        serializing_source.clone(),
        serializing_source,
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        serializing.output_file.as_deref() == Some("out.txt"),
        "a serialization-only container rule must not move the input name"
    );
    Ok(())
}
