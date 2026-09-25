//! Tests for `OrthoConfigDocs` IR generation.
//!
//! Document-level metadata: version, app identity, headings, discovery, the
//! Windows section, and JSON round-tripping. Per-field metadata lives in the
//! sibling `docs_ir_fields.rs`; both suites share `support/docs_ir_config.rs`.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{
    ConfigFormat, DocMetadata, ORTHO_DOCS_IR_VERSION, OrthoConfigDocs, SourceKind, ValueType,
};
use rstest::{fixture, rstest};

#[path = "support/docs_ir_config.rs"]
mod docs_ir_config;
use docs_ir_config::DocsConfig;

#[fixture]
fn docs_metadata() -> DocMetadata {
    DocsConfig::get_doc_metadata()
}

#[rstest]
fn test_basic_metadata(docs_metadata: DocMetadata) -> Result<()> {
    let metadata = docs_metadata;

    ensure!(
        metadata.ir_version == ORTHO_DOCS_IR_VERSION,
        "expected IR version {ORTHO_DOCS_IR_VERSION}, got {}",
        metadata.ir_version
    );
    ensure!(
        metadata.app_name == "demo-app",
        "expected app name demo-app, got {}",
        metadata.app_name
    );
    ensure!(
        metadata.bin_name.as_deref() == Some("demo-cli"),
        "expected bin name demo-cli, got {:?}",
        metadata.bin_name
    );
    ensure!(
        metadata.about_id == "demo-app.about",
        "expected default about_id, got {}",
        metadata.about_id
    );
    ensure!(
        metadata.synopsis_id.as_deref() == Some("demo.synopsis"),
        "expected synopsis_id demo.synopsis, got {:?}",
        metadata.synopsis_id
    );
    ensure!(
        metadata.subcommands.is_empty(),
        "expected no subcommands, got {}",
        metadata.subcommands.len()
    );
    Ok(())
}

#[rstest]
fn test_sections_headings(docs_metadata: DocMetadata) {
    let headings = &docs_metadata.sections.headings_ids;
    assert!(
        headings.options == "demo.headings.options",
        "expected options heading override, got {}",
        headings.options
    );
    assert!(
        headings.name == "ortho.headings.name",
        "expected default name heading, got {}",
        headings.name
    );
}

#[rstest]
fn test_sections_discovery(docs_metadata: DocMetadata) -> Result<()> {
    let discovery = docs_metadata
        .sections
        .discovery
        .as_ref()
        .ok_or_else(|| anyhow!("expected discovery metadata"))?;
    ensure!(
        discovery.override_flag_long.as_deref() == Some("config"),
        "expected override flag config, got {:?}",
        discovery.override_flag_long
    );
    ensure!(
        discovery.override_env.as_deref() == Some("DEMO_CONFIG"),
        "expected override env DEMO_CONFIG, got {:?}",
        discovery.override_env
    );
    ensure!(
        discovery.formats == vec![ConfigFormat::Yaml],
        "expected YAML format, got {:?}",
        discovery.formats
    );
    ensure!(
        discovery.search_paths.is_empty(),
        "expected no discovery paths yet"
    );
    ensure!(
        discovery.xdg_compliant == cfg!(any(unix, target_os = "redox")),
        "unexpected xdg_compliant value"
    );
    Ok(())
}

#[rstest]
fn test_windows_metadata(docs_metadata: DocMetadata) -> Result<()> {
    let windows = docs_metadata
        .windows
        .as_ref()
        .ok_or_else(|| anyhow!("expected windows metadata"))?;
    ensure!(
        windows.module_name.as_deref() == Some("Demo"),
        "expected module_name Demo, got {:?}",
        windows.module_name
    );
    ensure!(
        windows.export_aliases == vec!["demo"],
        "expected export_aliases demo, got {:?}",
        windows.export_aliases
    );
    ensure!(
        !windows.include_common_parameters,
        "expected include_common_parameters false"
    );
    ensure!(
        windows.split_subcommands_into_functions,
        "expected split_subcommands true"
    );
    ensure!(
        windows.help_info_uri.as_deref() == Some("https://example.com/help"),
        "expected help_info_uri, got {:?}",
        windows.help_info_uri
    );
    Ok(())
}

#[rstest]
fn test_json_serialization(docs_metadata: DocMetadata) -> Result<()> {
    let json = serde_json::to_string(&docs_metadata)?;
    ensure!(!json.is_empty(), "expected JSON output");
    Ok(())
}

#[rstest]
fn test_json_round_trip(docs_metadata: DocMetadata) -> Result<()> {
    let json = serde_json::to_string_pretty(&docs_metadata)?;
    let decoded: DocMetadata = serde_json::from_str(&json)?;
    ensure!(
        decoded == docs_metadata,
        "expected IR JSON round-trip to preserve metadata"
    );
    Ok(())
}

#[rstest]
fn test_json_deserializes_enum_variants() -> Result<()> {
    let string_value: ValueType = serde_json::from_str("\"String\"")?;
    ensure!(
        string_value == ValueType::String,
        "expected String ValueType"
    );

    let enum_value: ValueType =
        serde_json::from_str(r#"{"Enum":{"variants":["standard","debug"]}}"#)?;
    ensure!(
        enum_value
            == ValueType::Enum {
                variants: vec!["standard".to_owned(), "debug".to_owned()],
            },
        "expected Enum ValueType to deserialize"
    );

    let format: ConfigFormat = serde_json::from_str("\"Toml\"")?;
    ensure!(format == ConfigFormat::Toml, "expected Toml format");

    let source: SourceKind = serde_json::from_str("\"Env\"")?;
    ensure!(source == SourceKind::Env, "expected Env source kind");
    Ok(())
}
