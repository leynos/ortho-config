//! Tests for `OrthoConfigDocs` IR generation.

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use ortho_config::docs::{
    ConfigFormat, DocMetadata, ORTHO_DOCS_IR_VERSION, OrthoConfigDocs, SourceKind, ValueType,
};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    discovery(
        app_name = "demo-app",
        env_var = "DEMO_CONFIG",
        config_file_name = "config.yaml",
        config_cli_visible = true,
        config_cli_long = "config"
    ),
    synopsis_id = "demo.synopsis",
    bin_name = "demo-cli",
    headings(options = "demo.headings.options"),
    precedence(order = ["defaults", "file", "env", "cli"], rationale_id = "demo.precedence"),
    windows(
        module_name = "Demo",
        export_aliases = ["demo"],
        include_common_parameters = false,
        split_subcommands = true,
        help_info_uri = "https://example.com/help"
    )
)]
struct DocsConfig {
    #[ortho_config(
        help_id = "demo.fields.port.help",
        long_help_id = "demo.fields.port.long_help",
        value(type = "u16"),
        deprecated(note_id = "demo.fields.port.deprecated"),
        required,
        env(name = "DEMO_PORT"),
        file(key_path = "network.port"),
        cli(value_name = "PORT", hide_in_help)
    )]
    port: u16,
    #[serde(rename = "logLevel")]
    log_level: Option<String>,
    #[ortho_config(default = 3)]
    retries: u8,
    verbose: bool,
    /// Uses `serde(default)` but no `ortho_config(default)`; `required` should resolve to `false`.
    #[serde(default)]
    serde_default_only: String,
    /// Collection type without explicit `required`/`default`; collections default to non-required.
    collection_values: Vec<String>,
    /// Non-optional scalar where `resolve_required` would normally infer `required == true`,
    /// but the explicit `required = false` override should win.
    #[ortho_config(required = false)]
    explicitly_not_required: String,
}

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
fn test_sections_headings(docs_metadata: DocMetadata) -> Result<()> {
    let headings = &docs_metadata.sections.headings_ids;
    ensure!(
        headings.options == "demo.headings.options",
        "expected options heading override, got {}",
        headings.options
    );
    ensure!(
        headings.name == "ortho.headings.name",
        "expected default name heading, got {}",
        headings.name
    );
    Ok(())
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
        !verbose_cli.takes_value,
        "expected verbose to not take a value"
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
