//! Documentation IR schema used by `cargo-orthohelp`.
//!
//! Keep this in sync with `ortho_config::docs` so the tool can parse IR JSON
//! without depending on unpublished crate internals.
//!
//! The mirror follows the same grouping as `ortho_config::docs::ir`: the
//! document and its sections here, per-field metadata in `field`, behaviour
//! declarations in `behaviour`, and reusable content in `content`. The
//! submodules are private, so these names are code spans rather than links:
//! a public module naming private items would otherwise trip
//! `rustdoc::private-intra-doc-links`. Every type stays re-exported from this
//! module, so `crate::schema::DocMetadata` and friends remain valid paths for
//! the rest of the tool.

use serde::{Deserialize, Serialize};

mod behaviour;
mod content;
mod field;

pub use behaviour::{BehaviourMetadata, InteractionKind, MutationKind};
pub use content::{Example, Link, Note};
pub use field::{
    CliMetadata, DefaultValue, Deprecation, EnvMetadata, FieldMetadata, FileMetadata, ValueType,
};

/// Current IR schema version.
pub const ORTHO_DOCS_IR_VERSION: &str = "2.0";

/// Top-level documentation metadata for a configuration command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocMetadata {
    /// IR schema version string (for example, "2.0").
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
    pub subcommands: Vec<Self>,
    /// Optional Windows metadata for `PowerShell` help output.
    pub windows: Option<WindowsMetadata>,
    /// Optional declared execution behaviour for agent-native consumers.
    ///
    /// Absent (`None`) means the author made no declaration; consumers must
    /// treat the command as unknown rather than inferring semantics from
    /// names or flags.
    #[serde(default)]
    pub behaviour: Option<BehaviourMetadata>,
    /// Profile selection metadata for opted-in structs (roadmap 9.1.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<DocProfilesMeta>,
}

/// Profile selection metadata (decision D15).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocProfilesMeta {
    /// The generated flag name (no leading `--`).
    pub flag: String,
    /// The selector environment variable name.
    pub env_var: String,
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
    /// Fluent ID for the COMMANDS heading (inline subcommands).
    #[serde(default)]
    pub commands: Option<String>,
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
///
/// Mirrors `ortho_config::docs::SourceKind`; keep the two in sync. Variants
/// run from lowest to highest precedence, with `Profile` between `File` and
/// `Env` because a profile overlay is selected configuration data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceKind {
    /// Defaults supplied by the application.
    Defaults,
    /// Values loaded from configuration files.
    File,
    /// Values loaded from a selected profile overlay.
    Profile,
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

#[cfg(test)]
mod tests;
