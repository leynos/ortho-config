//! Intermediate representation (IR) types for OrthoConfig documentation.
//!
//! These structures are emitted by the derive macro and consumed by external
//! tooling such as `cargo-orthohelp` to generate user-facing documentation.

use serde::Serialize;

/// Top-level documentation metadata for a configuration command.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocMetadata {
    pub ir_version: String,
    pub app_name: String,
    pub bin_name: Option<String>,
    pub about_id: String,
    pub synopsis_id: Option<String>,
    pub sections: SectionsMetadata,
    pub fields: Vec<FieldMetadata>,
    pub subcommands: Vec<DocMetadata>,
    pub windows: Option<WindowsMetadata>,
}

/// Section-level metadata and supporting content.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SectionsMetadata {
    pub headings_ids: HeadingIds,
    pub discovery: Option<ConfigDiscoveryMeta>,
    pub precedence: Option<PrecedenceMeta>,
    pub examples: Vec<Example>,
    pub links: Vec<Link>,
    pub notes: Vec<Note>,
}

/// Fluent IDs for standard documentation headings.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HeadingIds {
    pub name: String,
    pub synopsis: String,
    pub description: String,
    pub options: String,
    pub environment: String,
    pub files: String,
    pub precedence: String,
    pub exit_status: String,
    pub examples: String,
    pub see_also: String,
}

/// Metadata describing a single configuration field.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FieldMetadata {
    pub name: String,
    pub help_id: String,
    pub long_help_id: Option<String>,
    pub value: Option<ValueType>,
    pub default: Option<DefaultValue>,
    pub required: bool,
    pub deprecated: Option<Deprecation>,
    pub cli: Option<CliMetadata>,
    pub env: Option<EnvMetadata>,
    pub file: Option<FileMetadata>,
    pub examples: Vec<Example>,
    pub links: Vec<Link>,
    pub notes: Vec<Note>,
}

/// CLI documentation metadata for a field.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CliMetadata {
    pub long: Option<String>,
    pub short: Option<char>,
    pub value_name: Option<String>,
    pub multiple: bool,
    pub takes_value: bool,
    pub possible_values: Vec<String>,
    pub hide_in_help: bool,
}

/// Environment variable documentation metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvMetadata {
    pub var_name: String,
}

/// File configuration metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileMetadata {
    pub key_path: String,
}

/// Strongly-typed value metadata for documentation rendering.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum ValueType {
    String,
    Integer { bits: u8, signed: bool },
    Float { bits: u8 },
    Bool,
    Duration,
    Path,
    IpAddr,
    Hostname,
    Url,
    Enum { variants: Vec<String> },
    List { of: Box<ValueType> },
    Map { of: Box<ValueType> },
    Custom { name: String },
}

/// Display value for a default.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DefaultValue {
    pub display: String,
}

/// Deprecation metadata for a field.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Deprecation {
    pub note_id: String,
}

/// Configuration discovery metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConfigDiscoveryMeta {
    pub formats: Vec<ConfigFormat>,
    pub search_paths: Vec<PathPattern>,
    pub override_flag_long: Option<String>,
    pub override_env: Option<String>,
    pub xdg_compliant: bool,
}

/// Supported configuration file formats.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum ConfigFormat {
    Toml,
    Yaml,
    Json,
}

/// Ordered search path pattern.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PathPattern {
    pub pattern: String,
    pub note_id: Option<String>,
}

/// Configuration source precedence metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PrecedenceMeta {
    pub order: Vec<SourceKind>,
    pub rationale_id: Option<String>,
}

/// Kinds of configuration sources.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SourceKind {
    Defaults,
    File,
    Env,
    Cli,
}

/// Optional Windows metadata for PowerShell help generation.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WindowsMetadata {
    pub module_name: Option<String>,
    pub export_aliases: Vec<String>,
    pub include_common_parameters: bool,
    pub split_subcommands_into_functions: bool,
    pub help_info_uri: Option<String>,
}

/// Documentation example snippet.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Example {
    pub title_id: Option<String>,
    pub code: String,
    pub body_id: Option<String>,
}

/// Related link metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Link {
    pub text_id: Option<String>,
    pub uri: String,
}

/// Documentation note metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Note {
    pub text_id: String,
}
