//! Per-field metadata assertions for `OrthoConfigDocs` IR generation.
//!
//! Split from `docs_ir.rs`, which covers document-level metadata, so that each
//! file stays under the repository's 400-line limit. `field_by_name` is the
//! shared lookup for the field assertions, and it lives here because this is
//! the only file that uses it; a second copy beside the document-level tests
//! would be dead code.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, OrthoConfigDocs, ValueType};
use rstest::{fixture, rstest};

#[path = "support/docs_ir_config.rs"]
mod docs_ir_config;
use docs_ir_config::DocsConfig;

#[fixture]
fn docs_metadata() -> DocMetadata {
    DocsConfig::get_doc_metadata()
}

#[rstest]
fn test_field_port(docs_metadata: DocMetadata) -> Result<()> {
    let port = field_by_name(&docs_metadata, "port")?;
    ensure!(
        port.help_id == "demo.fields.port.help",
        "expected port help_id override"
    );
    ensure!(
        port.long_help_id.as_deref() == Some("demo.fields.port.long_help"),
        "expected port long_help_id override"
    );
    ensure!(port.required, "expected port to be required");
    ensure!(
        port.deprecated.as_ref().map(|value| value.note_id.as_str())
            == Some("demo.fields.port.deprecated"),
        "expected port deprecated note"
    );
    ensure!(
        port.value
            == Some(ValueType::Integer {
                bits: 16,
                signed: false
            }),
        "expected port to be u16"
    );
    let port_cli = port
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("expected port CLI metadata"))?;
    ensure!(
        port_cli.long.as_deref() == Some("port"),
        "expected port long flag"
    );
    ensure!(port_cli.short == Some('p'), "expected port short flag");
    ensure!(
        port_cli.value_name.as_deref() == Some("PORT"),
        "expected port value name"
    );
    ensure!(port_cli.takes_value, "expected port takes_value true");
    ensure!(!port_cli.multiple, "expected port multiple false");
    ensure!(
        port_cli.possible_values.is_empty(),
        "expected no enum values"
    );
    ensure!(port_cli.hide_in_help, "expected port hidden in help");
    ensure!(
        port.env.as_ref().map(|value| value.var_name.as_str()) == Some("DEMO_PORT"),
        "expected port env name"
    );
    ensure!(
        port.file.as_ref().map(|value| value.key_path.as_str()) == Some("network.port"),
        "expected port file key"
    );
    Ok(())
}

#[rstest]
fn test_field_log_level(docs_metadata: DocMetadata) -> Result<()> {
    let log_level = field_by_name(&docs_metadata, "log_level")?;
    ensure!(
        log_level.help_id == "demo-app.fields.log_level.help",
        "expected log_level help_id default"
    );
    ensure!(
        log_level.long_help_id.as_deref() == Some("demo-app.fields.log_level.long_help"),
        "expected log_level long_help_id default"
    );
    ensure!(!log_level.required, "expected log_level optional");
    ensure!(
        log_level.value == Some(ValueType::String),
        "expected log_level string value"
    );
    ensure!(
        log_level.env.as_ref().map(|value| value.var_name.as_str()) == Some("APP_LOG_LEVEL"),
        "expected log_level env name"
    );
    ensure!(
        log_level.file.as_ref().map(|value| value.key_path.as_str()) == Some("logLevel"),
        "expected log_level file key"
    );
    Ok(())
}

#[rstest]
fn test_field_retries(docs_metadata: DocMetadata) -> Result<()> {
    let retries = field_by_name(&docs_metadata, "retries")?;
    ensure!(
        retries.default.as_ref().map(|value| value.display.as_str()) == Some("3"),
        "expected retries default display"
    );
    ensure!(
        retries.value
            == Some(ValueType::Integer {
                bits: 8,
                signed: false
            }),
        "expected retries u8 type"
    );
    Ok(())
}

#[rstest]
fn test_field_verbose(docs_metadata: DocMetadata) -> Result<()> {
    let verbose = field_by_name(&docs_metadata, "verbose")?;
    ensure!(
        verbose.value == Some(ValueType::Bool),
        "expected verbose boolean type"
    );
    let verbose_cli = verbose
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("expected verbose CLI metadata"))?;
    ensure!(
        verbose_cli.value_name.as_deref() == Some("BOOL"),
        "expected verbose value name BOOL, got {:?}",
        verbose_cli.value_name
    );
    ensure!(
        verbose_cli.takes_value,
        "expected verbose to accept an optional value"
    );
    ensure!(
        verbose_cli.value_optional,
        "expected verbose to mark its value optional"
    );
    ensure!(
        verbose_cli.possible_values == ["true", "false"],
        "expected verbose possible values true/false, got {:?}",
        verbose_cli.possible_values
    );
    Ok(())
}

/// Tests that `#[serde(default)]` without `#[ortho_config(default)]` resolves to non-required.
#[rstest]
fn test_field_serde_default_only(docs_metadata: DocMetadata) -> Result<()> {
    let field = field_by_name(&docs_metadata, "serde_default_only")?;
    ensure!(
        !field.required,
        "expected serde_default_only to be non-required due to serde(default)"
    );
    ensure!(
        field.default.is_none(),
        "expected no explicit default from ortho_config for serde_default_only"
    );
    Ok(())
}

/// Tests that collection types without explicit `required`/`default` resolve to non-required.
#[rstest]
fn test_field_collection_values(docs_metadata: DocMetadata) -> Result<()> {
    let field = field_by_name(&docs_metadata, "collection_values")?;
    ensure!(
        !field.required,
        "expected collection_values to be non-required as a Vec type"
    );
    Ok(())
}

/// Tests that explicit `required = false` overrides the inferred value.
#[rstest]
fn test_field_explicitly_not_required(docs_metadata: DocMetadata) -> Result<()> {
    let field = field_by_name(&docs_metadata, "explicitly_not_required")?;
    ensure!(
        !field.required,
        "expected explicitly_not_required to be non-required due to explicit override"
    );
    Ok(())
}

fn field_by_name<'a>(
    metadata: &'a ortho_config::docs::DocMetadata,
    name: &'a str,
) -> Result<&'a ortho_config::docs::FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| anyhow!("missing field {name}"))
}
