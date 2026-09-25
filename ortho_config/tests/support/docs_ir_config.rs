//! Shared `DocsConfig` for the `docs_ir` test suites.
//!
//! The IR assertions split across `docs_ir.rs`, which checks document-level
//! metadata, and `docs_ir_fields.rs`, which checks per-field metadata. Both
//! describe the same configuration, whose 48-line `#[ortho_config(...)]`
//! attribute names every feature the IR can report. Two copies of that
//! attribute would drift apart the first time a feature was added, so it lives
//! here instead, and each suite keeps its own one-line fixture: `rstest`
//! resolves a fixture by name, and importing one from an ancestor module is
//! both unnecessary and reported as an unused import.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Configuration exercising the metadata surface the IR exposes.
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
pub struct DocsConfig {
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
