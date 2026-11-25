//! End-to-end checks that derive-emitted metadata is already projected.
//!
//! The projection is applied by the generated `get_doc_metadata*` methods, so
//! these tests exercise the macro, the shared enum, and serde-visible labels
//! together rather than calling the library helper directly.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, FieldMetadata, OrthoConfigDocs, ProfileFieldExposure};
use ortho_config::{MapEnv, OrthoConfig};

/// Configuration with one ordinary and two protected fields.
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    clap::Parser,
    OrthoConfig,
)]
#[ortho_config(prefix = "DEMO_")]
struct RedactionConfig {
    /// Public port whose default must remain visible.
    #[arg(long, default_value_t = 8080)]
    #[ortho_config(default = 8080)]
    port: u16,

    /// Confidential token.
    #[arg(long, default_value = "TOKEN-SENTINEL")]
    #[ortho_config(profile_field = "secret")]
    token: String,

    /// Locator the application resolves elsewhere.
    #[arg(long, default_value = "LOCATOR-SENTINEL")]
    #[ortho_config(profile_field = "reference_only")]
    locator: String,
}

/// Looks up one field in a metadata document.
fn field<'a>(metadata: &'a DocMetadata, name: &'a str) -> Result<&'a FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .with_context(|| format!("missing field {name}"))
}

#[test]
fn ordinary_field_keeps_its_default() -> Result<()> {
    let metadata = RedactionConfig::get_doc_metadata();
    let port = field(&metadata, "port")?;

    ensure!(port.profile_field.is_none(), "port is not protected");
    ensure!(port.default.is_some(), "the public default stays visible");
    Ok(())
}

#[test]
fn protected_fields_lose_their_literals() -> Result<()> {
    let metadata = RedactionConfig::get_doc_metadata();

    for (name, expected) in [
        ("token", ProfileFieldExposure::Secret),
        ("locator", ProfileFieldExposure::ReferenceOnly),
    ] {
        let protected = field(&metadata, name)?;
        ensure!(
            protected.profile_field == Some(expected),
            "{name} keeps its label"
        );
        ensure!(protected.default.is_none(), "{name} has no default");
        ensure!(protected.examples.is_empty(), "{name} has no examples");
        let cli = protected
            .cli
            .as_ref()
            .with_context(|| format!("{name} has CLI metadata"))?;
        ensure!(
            cli.possible_values.is_empty(),
            "{name} has no possible values"
        );
        ensure!(
            cli.long.as_deref() == Some(name),
            "{name} keeps its public CLI spelling"
        );
    }
    Ok(())
}

#[test]
fn path_sensitive_metadata_is_projected_too() -> Result<()> {
    let metadata = RedactionConfig::get_doc_metadata_for_path(&["demo".to_owned()]);

    let token = field(&metadata, "token")?;

    ensure!(token.default.is_none(), "path variant redacts defaults");
    ensure!(
        token.profile_field == Some(ProfileFieldExposure::Secret),
        "path variant keeps the label"
    );
    Ok(())
}

/// The same fields without any disclosure classification.
///
/// V5 compares this struct with [`RedactionConfig`] under identical sources:
/// marking a field must change no load result.
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    clap::Parser,
    OrthoConfig,
)]
#[ortho_config(prefix = "DEMO_")]
struct UnmarkedConfig {
    /// Public port whose default must remain visible.
    #[arg(long, default_value_t = 8080)]
    #[ortho_config(default = 8080)]
    port: u16,

    /// Confidential token.
    #[arg(long, default_value = "TOKEN-SENTINEL")]
    token: String,

    /// Locator the application resolves elsewhere.
    #[arg(long, default_value = "LOCATOR-SENTINEL")]
    locator: String,
}

/// Builds an injected environment layer supplying every field.
fn env_source(token: &str) -> Arc<MapEnv> {
    Arc::new(
        MapEnv::new()
            .with_var("DEMO_TOKEN", token)
            .with_var("DEMO_LOCATOR", "LOADED-LOCATOR")
            .with_var("DEMO_PORT", "9090"),
    )
}

#[test]
fn marking_a_field_changes_no_load_result() -> Result<()> {
    let marked_source = env_source("LOADED-TOKEN");
    let marked = RedactionConfig::load_from_iter_with_sources(
        ["demo"],
        marked_source.clone(),
        marked_source,
    )
    .map_err(|err| anyhow!(err))?;

    let unmarked_source = env_source("LOADED-TOKEN");
    let unmarked = UnmarkedConfig::load_from_iter_with_sources(
        ["demo"],
        unmarked_source.clone(),
        unmarked_source,
    )
    .map_err(|err| anyhow!(err))?;

    ensure!(marked.token == "LOADED-TOKEN", "the protected value loads");
    ensure!(
        marked.token == unmarked.token,
        "classification must not change the loaded token"
    );
    ensure!(marked.port == unmarked.port, "ordinary fields agree");
    ensure!(marked.locator == unmarked.locator, "two labels agree");
    Ok(())
}

#[test]
fn protected_field_keeps_cli_precedence_over_environment() -> Result<()> {
    let source = env_source("ENV-TOKEN");
    let cfg = RedactionConfig::load_from_iter_with_sources(
        ["demo", "--token", "CLI-TOKEN"],
        source.clone(),
        source,
    )
    .map_err(|err| anyhow!(err))?;

    ensure!(
        cfg.token == "CLI-TOKEN",
        "the CLI still outranks the environment for a protected field"
    );
    Ok(())
}
