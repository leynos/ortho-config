//! Documentation IR schema used by `cargo-orthohelp`.
//!
//! Keep this in sync with `ortho_config::docs` so the tool can parse IR JSON
//! without depending on unpublished crate internals.

use serde::{Deserialize, Serialize};

/// Current IR schema version.
pub const ORTHO_DOCS_IR_VERSION: &str = "1.1";

/// Top-level documentation metadata for a configuration command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocMetadata {
    /// IR schema version string (for example, "1.1").
    pub ir_version: String,
    /// Application name used for display and identifier generation.
    pub app_name: String,
    /// Optional override for the binary name used in docs output.
    pub bin_name: Option<String>,
    /// Fluent ID describing the command overview.
    pub about_id: String,
    /// Optional Fluent ID for the synopsis line.
    pub synopsis_id: Option<String>,
    /// Section metadata for headings, discovery, and extras.
    pub sections: SectionsMetadata,
    /// Field-level documentation metadata.
    pub fields: Vec<FieldMetadata>,
    /// Nested subcommand metadata.
    pub subcommands: Vec<DocMetadata>,
    /// Optional Windows metadata for `PowerShell` help output.
    pub windows: Option<WindowsMetadata>,
}

/// Section-level metadata and supporting content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionsMetadata {
    /// Fluent IDs for standard documentation headings.
    pub headings_ids: HeadingIds,
    /// Optional configuration discovery metadata.
    pub discovery: Option<ConfigDiscoveryMeta>,
    /// Optional source precedence metadata.
    pub precedence: Option<PrecedenceMeta>,
    /// Examples rendered at the command level.
    pub examples: Vec<Example>,
    /// Related links rendered at the command level.
    pub links: Vec<Link>,
    /// Notes rendered at the command level.
    pub notes: Vec<Note>,
}

/// Fluent IDs for standard documentation headings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadingIds {
    /// Fluent ID for the NAME heading.
    pub name: String,
    /// Fluent ID for the SYNOPSIS heading.
    pub synopsis: String,
    /// Fluent ID for the DESCRIPTION heading.
    pub description: String,
    /// Fluent ID for the OPTIONS heading.
    pub options: String,
    /// Fluent ID for the ENVIRONMENT heading.
    pub environment: String,
    /// Fluent ID for the FILES heading.
    pub files: String,
    /// Fluent ID for the PRECEDENCE heading.
    pub precedence: String,
    /// Fluent ID for the EXIT STATUS heading.
    pub exit_status: String,
    /// Fluent ID for the EXAMPLES heading.
    pub examples: String,
    /// Fluent ID for the SEE ALSO heading.
    pub see_also: String,
}

/// Metadata describing a single configuration field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldMetadata {
    /// Rust field name for the setting.
    pub name: String,
    /// Fluent ID for the short help text.
    pub help_id: String,
    /// Optional Fluent ID for long-form help text.
    pub long_help_id: Option<String>,
    /// Optional semantic value type for formatting.
    pub value: Option<ValueType>,
    /// Optional default value display string.
    pub default: Option<DefaultValue>,
    /// Whether the field is required.
    pub required: bool,
    /// Deprecation metadata, if any.
    pub deprecated: Option<Deprecation>,
    /// CLI-related documentation metadata.
    pub cli: Option<CliMetadata>,
    /// Environment-variable documentation metadata.
    pub env: Option<EnvMetadata>,
    /// File configuration documentation metadata.
    pub file: Option<FileMetadata>,
    /// Field-level examples.
    pub examples: Vec<Example>,
    /// Field-level related links.
    pub links: Vec<Link>,
    /// Field-level notes.
    pub notes: Vec<Note>,
}

/// CLI documentation metadata for a field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliMetadata {
    /// Long CLI flag name (without the leading dashes).
    pub long: Option<String>,
    /// Optional short CLI flag character.
    pub short: Option<char>,
    /// Optional CLI value placeholder.
    pub value_name: Option<String>,
    /// Whether the field accepts repeated values.
    pub multiple: bool,
    /// Whether the CLI flag takes a value (false for switches).
    pub takes_value: bool,
    /// Allowed values for enum-like options.
    pub possible_values: Vec<String>,
    /// Whether the flag is hidden from help output.
    pub hide_in_help: bool,
}

/// Environment variable documentation metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvMetadata {
    /// Environment variable name.
    pub var_name: String,
}

/// File configuration metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMetadata {
    /// Dotted configuration key path.
    pub key_path: String,
}

/// Strongly-typed value metadata for documentation rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValueType {
    /// String value.
    String,
    /// Integer value with bit-width and sign.
    Integer {
        /// Integer bit width.
        bits: u8,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// Floating point value with bit-width.
    Float {
        /// Floating point bit width.
        bits: u8,
    },
    /// Boolean switch.
    Bool,
    /// Duration value.
    Duration,
    /// File system path.
    Path,
    /// IP address.
    IpAddr,
    /// Hostname value.
    Hostname,
    /// URL value.
    Url,
    /// Enumeration of allowed values.
    Enum {
        /// Allowed variants for the enum.
        variants: Vec<String>,
    },
    /// List of nested values.
    List {
        /// Element type for list values.
        of: Box<ValueType>,
    },
    /// Map with value type metadata.
    Map {
        /// Value type for map entries.
        of: Box<ValueType>,
    },
    /// Custom domain-specific value type.
    Custom {
        /// Human-readable type name.
        name: String,
    },
}

/// Display value for a default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultValue {
    /// Human-readable default string.
    pub display: String,
}

/// Deprecation metadata for a field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Deprecation {
    /// Fluent ID for the deprecation note.
    pub note_id: String,
}

/// Configuration discovery metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDiscoveryMeta {
    /// Supported configuration file formats.
    pub formats: Vec<ConfigFormat>,
    /// Ordered search path patterns.
    pub search_paths: Vec<PathPattern>,
    /// Long CLI flag that overrides the config path.
    pub override_flag_long: Option<String>,
    /// Environment variable that overrides the config path.
    pub override_env: Option<String>,
    /// Whether discovery follows XDG semantics.
    pub xdg_compliant: bool,
}

/// Supported configuration file formats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML configuration format.
    Toml,
    /// YAML configuration format.
    Yaml,
    /// JSON configuration format.
    Json,
}

/// Ordered search path pattern.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathPattern {
    /// Path pattern template.
    pub pattern: String,
    /// Optional Fluent ID describing the pattern.
    pub note_id: Option<String>,
}

/// Configuration source precedence metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrecedenceMeta {
    /// Ordered list of source kinds, from lowest to highest precedence.
    pub order: Vec<SourceKind>,
    /// Optional Fluent ID describing precedence rationale.
    pub rationale_id: Option<String>,
}

/// Kinds of configuration sources.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceKind {
    /// Defaults supplied by the application.
    Defaults,
    /// Values loaded from configuration files.
    File,
    /// Values loaded from environment variables.
    Env,
    /// Values loaded from CLI arguments.
    Cli,
}

/// Optional Windows metadata for `PowerShell` help generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsMetadata {
    /// Module name used for `PowerShell` help output.
    pub module_name: Option<String>,
    /// Aliases exported by the wrapper module.
    pub export_aliases: Vec<String>,
    /// Whether `PowerShell` common parameters are included.
    pub include_common_parameters: bool,
    /// Whether subcommands are split into wrapper functions.
    pub split_subcommands_into_functions: bool,
    /// Optional `HelpInfoUri` for Update-Help.
    pub help_info_uri: Option<String>,
}

/// Documentation example snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Example {
    /// Optional Fluent ID for the example title.
    pub title_id: Option<String>,
    /// Example code snippet.
    pub code: String,
    /// Optional Fluent ID for the example body text.
    pub body_id: Option<String>,
}

/// Related link metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    /// Optional Fluent ID for the link text.
    pub text_id: Option<String>,
    /// URI associated with the link.
    pub uri: String,
}

/// Documentation note metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    /// Fluent ID for the note content.
    pub text_id: String,
}

#[cfg(test)]
mod tests;
