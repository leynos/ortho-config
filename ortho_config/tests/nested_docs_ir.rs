//! Tests for rich nested command trees in `OrthoConfigDocs` IR generation.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, OrthoConfigDocs, ValueType};
use rstest::{fixture, rstest};

#[path = "rstest_bdd/nested_docs_fixture.rs"]
mod nested_docs_fixture;

use nested_docs_fixture::NestedDocsConfig;

#[fixture]
fn nested_metadata() -> DocMetadata {
    NestedDocsConfig::get_doc_metadata()
}

#[rstest]
fn nested_root_lists_all_top_level_commands_in_declaration_order(
    nested_metadata: DocMetadata,
) -> Result<()> {
    let names = command_names(&nested_metadata);

    ensure!(
        names == ["greet", "version", "admin"],
        "expected top-level commands in declaration order, got {names:?}",
    );
    Ok(())
}

#[rstest]
fn nested_root_fields_excludes_subcommand_selector(nested_metadata: DocMetadata) -> Result<()> {
    let field_names = field_names(&nested_metadata);

    ensure!(
        field_names == ["global"],
        "expected only the global field in root metadata, got {field_names:?}",
    );
    Ok(())
}

#[rstest]
fn nested_greet_command_has_expected_fields_and_examples(
    nested_metadata: DocMetadata,
) -> Result<()> {
    let greet = command_by_name(&nested_metadata, "greet")?;
    let recipient = field_by_name(greet, "recipient")?;

    ensure!(greet.app_name == "greet", "expected greet command metadata");
    ensure!(
        field_names(greet) == ["excited", "recipient"],
        "expected greet fields to include its flag and value",
    );
    ensure!(
        recipient
            .default
            .as_ref()
            .map(|value| value.display.as_str())
            == Some("String :: from(\"World\")"),
        "expected recipient default display, got {:?}",
        recipient.default,
    );
    ensure!(
        recipient.value == Some(ValueType::String),
        "expected recipient string value type",
    );
    ensure!(
        recipient.cli.as_ref().and_then(|cli| cli.long.as_deref()) == Some("recipient"),
        "expected recipient long flag",
    );
    ensure!(
        recipient.env.as_ref().map(|env| env.var_name.as_str()) == Some("NESTED_APP_RECIPIENT"),
        "expected recipient environment variable",
    );
    ensure!(
        greet.sections.examples.iter().any(|example| {
            example.code == "nested-app greet --recipient Ada"
                && example.title_id.as_deref() == Some("nested-app.examples.greet.title")
        }),
        "expected greet command example",
    );
    Ok(())
}

#[rstest]
fn nested_version_command_has_no_fields(nested_metadata: DocMetadata) -> Result<()> {
    let version = command_by_name(&nested_metadata, "version")?;

    ensure!(
        version.fields.is_empty(),
        "expected version command to have no fields, got {:?}",
        field_names(version),
    );
    Ok(())
}

#[rstest]
fn nested_admin_command_lists_audit_and_grant_access(nested_metadata: DocMetadata) -> Result<()> {
    let admin = command_by_name(&nested_metadata, "admin")?;
    let nested_names = command_names(admin);

    ensure!(
        nested_names == ["audit", "grant-access"],
        "expected admin commands in declaration order, got {nested_names:?}",
    );
    Ok(())
}

#[rstest]
fn nested_admin_command_carries_split_subcommands_windows_metadata(
    nested_metadata: DocMetadata,
) -> Result<()> {
    let admin = command_by_name(&nested_metadata, "admin")?;
    let windows = admin
        .windows
        .as_ref()
        .ok_or_else(|| anyhow!("missing admin Windows metadata"))?;

    ensure!(
        windows.split_subcommands_into_functions,
        "expected admin Windows metadata to split subcommands",
    );
    Ok(())
}

#[rstest]
fn nested_greet_command_has_no_windows_metadata(nested_metadata: DocMetadata) -> Result<()> {
    let greet = command_by_name(&nested_metadata, "greet")?;

    ensure!(
        greet.windows.is_none(),
        "expected greet command to omit Windows metadata",
    );
    Ok(())
}

#[rstest]
fn nested_admin_audit_has_inherited_fluent_id_pattern(nested_metadata: DocMetadata) -> Result<()> {
    let admin = command_by_name(&nested_metadata, "admin")?;
    let audit = command_by_name(admin, "audit")?;
    let dry_run = field_by_name(audit, "dry_run")?;

    ensure!(
        audit.about_id == "audit.about",
        "expected audit about_id default, got {}",
        audit.about_id,
    );
    ensure!(
        dry_run.help_id == "audit.fields.dry_run.help",
        "expected audit field help_id default, got {}",
        dry_run.help_id,
    );
    Ok(())
}

/// Finds a child command by application name within the supplied metadata.
///
/// Returns the matching command metadata, or an error when `name` is absent.
fn command_by_name<'metadata>(
    metadata: &'metadata DocMetadata,
    name: &str,
) -> Result<&'metadata DocMetadata> {
    metadata
        .subcommands
        .iter()
        .find(|entry| entry.app_name == name)
        .ok_or_else(|| anyhow!("missing command metadata for {name}"))
}

/// Finds a field by Rust field name within the supplied command metadata.
///
/// Returns the matching field metadata, or an error when `name` is absent.
fn field_by_name<'metadata>(
    metadata: &'metadata DocMetadata,
    name: &str,
) -> Result<&'metadata ortho_config::docs::FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| anyhow!("missing field metadata for {name}"))
}

/// Lists child command application names in metadata order.
fn command_names(metadata: &DocMetadata) -> Vec<&str> {
    metadata
        .subcommands
        .iter()
        .map(|entry| entry.app_name.as_str())
        .collect()
}

/// Lists field names in metadata order.
fn field_names(metadata: &DocMetadata) -> Vec<&str> {
    metadata
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect()
}
