//! Output shapes for localized documentation.
//!
//! Localization preserves the document's structure and replaces only text, so
//! these types mirror [`crate::schema`] one level at a time instead of sharing
//! its types. A shared type would have to carry either resolved or unresolved
//! text, and every renderer would then have to distinguish the two.

use serde::Serialize;

use crate::schema::{
    CliMetadata, ConfigFormat, DefaultValue, EnvMetadata, FileMetadata, OutputMetadata,
    RendererMetadata, SourceKind, ValueType, WindowsMetadata,
};

/// Localized documentation metadata resolved for a single locale.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedDocMetadata {
    /// IR schema version.
    pub ir_version: String,
    /// BCP-47 locale identifier.
    pub locale: String,
    /// Application name.
    pub app_name: String,
    /// Binary name override.
    pub bin_name: Option<String>,
    /// Localized application description.
    pub about: String,
    /// Localized synopsis text.
    pub synopsis: Option<String>,
    /// Standard section metadata.
    pub sections: LocalizedSectionsMetadata,
    /// Configuration field definitions.
    pub fields: Vec<LocalizedFieldMetadata>,
    /// Subcommand definitions.
    pub subcommands: Vec<Self>,
    /// Windows-specific metadata.
    pub windows: Option<WindowsMetadata>,
    /// Declared renderer metadata, cloned unchanged from the source document.
    ///
    /// The renderer block describes presentation rather than text, so
    /// localization must copy it verbatim: dropping it here would remove the
    /// declaration from every published per-locale artefact even though the
    /// agent-context bridge still reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<RendererMetadata>,
    /// Declared structured-output metadata, cloned unchanged from the source.
    ///
    /// Like the renderer block, output capability is not text, so localization
    /// copies it verbatim. Dropping it here would remove the declaration from
    /// every published per-locale artefact while the bridge still reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputMetadata>,
}

/// Section metadata with resolved headings and content.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedSectionsMetadata {
    /// Localized heading labels.
    pub headings: LocalizedHeadings,
    /// Configuration file discovery metadata.
    pub discovery: Option<LocalizedConfigDiscoveryMeta>,
    /// Source precedence metadata.
    pub precedence: Option<LocalizedPrecedenceMeta>,
    /// Example snippets.
    pub examples: Vec<LocalizedExample>,
    /// Related links.
    pub links: Vec<LocalizedLink>,
    /// Additional notes.
    pub notes: Vec<LocalizedNote>,
}

/// Localized heading labels for standard sections.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedHeadings {
    /// NAME section heading.
    pub name: String,
    /// SYNOPSIS section heading.
    pub synopsis: String,
    /// DESCRIPTION section heading.
    pub description: String,
    /// OPTIONS section heading.
    pub options: String,
    /// ENVIRONMENT section heading.
    pub environment: String,
    /// FILES section heading.
    pub files: String,
    /// PRECEDENCE section heading.
    pub precedence: String,
    /// EXIT STATUS section heading.
    pub exit_status: String,
    /// EXAMPLES section heading.
    pub examples: String,
    /// SEE ALSO section heading.
    pub see_also: String,
    /// COMMANDS section heading (for inline subcommands).
    pub commands: String,
}

/// Field metadata with resolved help text.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedFieldMetadata {
    /// Field name identifier.
    pub name: String,
    /// Localized short help text.
    pub help: String,
    /// Localized detailed help text.
    pub long_help: Option<String>,
    /// Value type descriptor.
    pub value: Option<ValueType>,
    /// Default value.
    pub default: Option<DefaultValue>,
    /// Whether the field is required.
    pub required: bool,
    /// Deprecation metadata.
    pub deprecated: Option<LocalizedDeprecation>,
    /// CLI argument metadata.
    pub cli: Option<CliMetadata>,
    /// Environment variable metadata.
    pub env: Option<EnvMetadata>,
    /// Configuration file metadata.
    pub file: Option<FileMetadata>,
    /// Field-specific examples.
    pub examples: Vec<LocalizedExample>,
    /// Field-related links.
    pub links: Vec<LocalizedLink>,
    /// Field-specific notes.
    pub notes: Vec<LocalizedNote>,
}

/// Deprecation metadata with resolved notes.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedDeprecation {
    /// Localized deprecation notice.
    pub note: String,
}

/// Localized configuration discovery metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedConfigDiscoveryMeta {
    /// Supported configuration formats.
    pub formats: Vec<ConfigFormat>,
    /// Paths searched for configuration files.
    pub search_paths: Vec<LocalizedPathPattern>,
    /// Long flag to override config path.
    pub override_flag_long: Option<String>,
    /// Environment variable to override config path.
    pub override_env: Option<String>,
    /// Whether XDG Base Directories are used.
    pub xdg_compliant: bool,
}

/// Localized path pattern metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedPathPattern {
    /// Path pattern with optional environment variable references.
    pub pattern: String,
    /// Localized note explaining the path.
    pub note: Option<String>,
}

/// Localized source precedence metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedPrecedenceMeta {
    /// Order of configuration sources from lowest to highest priority.
    pub order: Vec<SourceKind>,
    /// Localized explanation of the precedence rules.
    pub rationale: Option<String>,
}

/// Localized documentation example snippet.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedExample {
    /// Localized title for the example.
    pub title: Option<String>,
    /// Code snippet text.
    pub code: String,
    /// Localized description of the example.
    pub body: Option<String>,
}

/// Localized link metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedLink {
    /// Localized link text.
    pub text: Option<String>,
    /// Link destination URI.
    pub uri: String,
}

/// Localized note metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedNote {
    /// Localized note text.
    pub text: String,
}
