//! Intermediate representation (IR) types for `OrthoConfig` documentation.
//!
//! These structures are emitted by the derive macro and consumed by external
//! tooling such as `cargo-orthohelp` to generate user-facing documentation.
//!
//! The types are grouped by the part of a command they describe: the document
//! as a whole and its sections here, per-field metadata in [`field`], declared
//! behaviour in [`behaviour`], and reusable content in [`content`]. This module
//! is private; it re-exports every type so `docs` presents the same flat
//! surface it did before the split, and the public paths stay
//! `ortho_config::docs::DocMetadata` and friends.

use serde::{Deserialize, Serialize};

mod behaviour;
mod content;
mod field;

pub use behaviour::{BehaviourMetadata, InteractionKind, MutationKind};
pub use content::{Example, Link, Note};
pub use field::{
    CliMetadata, DefaultValue, Deprecation, EnvMetadata, FieldMetadata, FileMetadata, ValueType,
};

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

/// Profile selection metadata carried in the documentation IR (decision D15).
///
/// Omitted when absent so the IR stays additive; the derive emits it only for
/// structs opting into `#[ortho_config(profiles)]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocProfilesMeta {
    /// The generated flag name following the `AgentInput::long` convention
    /// (no leading `--`).
    pub flag: String,
    /// The selector environment variable name (for example `APP_PROFILE`).
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
/// The variants follow the merge order reported by
/// [`MergeProvenance`](crate::declarative::MergeProvenance), from lowest to
/// highest precedence. `Profile` sits between `File` and `Env` because a
/// profile overlay is selected from the file chain and is therefore still
/// configuration data: the environment and the command line both outrank it.
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
