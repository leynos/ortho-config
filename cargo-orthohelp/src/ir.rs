//! Localized IR structures for `cargo-orthohelp`.

use ortho_config::{LanguageIdentifier, Localizer};
use serde::Serialize;

use crate::schema::{
    CliMetadata, ConfigDiscoveryMeta, ConfigFormat, DefaultValue, DocMetadata, EnvMetadata,
    Example, FieldMetadata, FileMetadata, Link, Note, PrecedenceMeta, SectionsMetadata, SourceKind,
    ValueType, WindowsMetadata,
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
    pub subcommands: Vec<LocalizedDocMetadata>,
    /// Windows-specific metadata.
    pub windows: Option<WindowsMetadata>,
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

/// Resolves Fluent IDs in `DocMetadata` into localized strings.
pub fn localize_doc(
    metadata: &DocMetadata,
    locale: &LanguageIdentifier,
    localizer: &dyn Localizer,
) -> LocalizedDocMetadata {
    LocalizedDocMetadata {
        ir_version: metadata.ir_version.clone(),
        locale: locale.to_string(),
        app_name: metadata.app_name.clone(),
        bin_name: metadata.bin_name.clone(),
        about: resolve_message(localizer, &metadata.about_id),
        synopsis: resolve_optional(localizer, metadata.synopsis_id.as_deref()),
        sections: localize_sections(&metadata.sections, localizer),
        fields: metadata
            .fields
            .iter()
            .map(|field| localize_field(field, localizer))
            .collect(),
        subcommands: metadata
            .subcommands
            .iter()
            .map(|command| localize_doc(command, locale, localizer))
            .collect(),
        windows: metadata.windows.clone(),
    }
}

/// Resolves section labels and optional content while cloning locale-independent metadata.
fn localize_sections(
    sections: &SectionsMetadata,
    localizer: &dyn Localizer,
) -> LocalizedSectionsMetadata {
    LocalizedSectionsMetadata {
        headings: LocalizedHeadings {
            name: resolve_heading(localizer, &sections.headings_ids.name),
            synopsis: resolve_heading(localizer, &sections.headings_ids.synopsis),
            description: resolve_heading(localizer, &sections.headings_ids.description),
            options: resolve_heading(localizer, &sections.headings_ids.options),
            environment: resolve_heading(localizer, &sections.headings_ids.environment),
            files: resolve_heading(localizer, &sections.headings_ids.files),
            precedence: resolve_heading(localizer, &sections.headings_ids.precedence),
            exit_status: resolve_heading(localizer, &sections.headings_ids.exit_status),
            examples: resolve_heading(localizer, &sections.headings_ids.examples),
            see_also: resolve_heading(localizer, &sections.headings_ids.see_also),
            commands: sections.headings_ids.commands.as_deref().map_or_else(
                || "COMMANDS".to_owned(),
                |id| resolve_heading(localizer, id),
            ),
        },
        discovery: sections
            .discovery
            .as_ref()
            .map(|discovery| localize_discovery(discovery, localizer)),
        precedence: sections
            .precedence
            .as_ref()
            .map(|precedence| localize_precedence(precedence, localizer)),
        examples: sections
            .examples
            .iter()
            .map(|example| localize_example(example, localizer))
            .collect(),
        links: sections
            .links
            .iter()
            .map(|link| localize_link(link, localizer))
            .collect(),
        notes: sections
            .notes
            .iter()
            .map(|note| localize_note(note, localizer))
            .collect(),
    }
}

/// Resolves translated field text and retains the source field's typed configuration metadata.
fn localize_field(field: &FieldMetadata, localizer: &dyn Localizer) -> LocalizedFieldMetadata {
    LocalizedFieldMetadata {
        name: field.name.clone(),
        help: resolve_message(localizer, &field.help_id),
        long_help: resolve_optional(localizer, field.long_help_id.as_deref()),
        value: field.value.clone(),
        default: field.default.clone(),
        required: field.required,
        deprecated: field
            .deprecated
            .as_ref()
            .map(|deprecated| LocalizedDeprecation {
                note: resolve_message(localizer, &deprecated.note_id),
            }),
        cli: field.cli.clone(),
        env: field.env.clone(),
        file: field.file.clone(),
        examples: field
            .examples
            .iter()
            .map(|example| localize_example(example, localizer))
            .collect(),
        links: field
            .links
            .iter()
            .map(|link| localize_link(link, localizer))
            .collect(),
        notes: field
            .notes
            .iter()
            .map(|note| localize_note(note, localizer))
            .collect(),
    }
}

/// Copies discovery mechanics unchanged and resolves only path-pattern notes.
fn localize_discovery(
    discovery: &ConfigDiscoveryMeta,
    localizer: &dyn Localizer,
) -> LocalizedConfigDiscoveryMeta {
    LocalizedConfigDiscoveryMeta {
        formats: discovery.formats.clone(),
        search_paths: discovery
            .search_paths
            .iter()
            .map(|pattern| LocalizedPathPattern {
                pattern: pattern.pattern.clone(),
                note: resolve_optional(localizer, pattern.note_id.as_deref()),
            })
            .collect(),
        override_flag_long: discovery.override_flag_long.clone(),
        override_env: discovery.override_env.clone(),
        xdg_compliant: discovery.xdg_compliant,
    }
}

/// Preserves source order and resolves the optional explanation for that precedence.
fn localize_precedence(
    precedence: &PrecedenceMeta,
    localizer: &dyn Localizer,
) -> LocalizedPrecedenceMeta {
    LocalizedPrecedenceMeta {
        order: precedence.order.clone(),
        rationale: resolve_optional(localizer, precedence.rationale_id.as_deref()),
    }
}

/// Resolves optional example labels while preserving code verbatim for display.
fn localize_example(example: &Example, localizer: &dyn Localizer) -> LocalizedExample {
    LocalizedExample {
        title: resolve_optional(localizer, example.title_id.as_deref()),
        code: example.code.clone(),
        body: resolve_optional(localizer, example.body_id.as_deref()),
    }
}

/// Resolves an optional link label while retaining its destination URI unchanged.
fn localize_link(link: &Link, localizer: &dyn Localizer) -> LocalizedLink {
    LocalizedLink {
        text: resolve_optional(localizer, link.text_id.as_deref()),
        uri: link.uri.clone(),
    }
}

/// Resolves a required note ID, representing a missing translation with the standard marker.
fn localize_note(note: &Note, localizer: &dyn Localizer) -> LocalizedNote {
    LocalizedNote {
        text: resolve_message(localizer, &note.text_id),
    }
}

/// Returns a translation or an explicit marker so unresolved required copy stays visible.
fn resolve_message(localizer: &dyn Localizer, id: &str) -> String {
    localizer
        .lookup(id, None)
        .unwrap_or_else(|| format!("[missing: {id}]"))
}

/// Uses a translation, then a built-in standard heading, and marks other missing IDs.
fn resolve_heading(localizer: &dyn Localizer, id: &str) -> String {
    localizer.lookup(id, None).unwrap_or_else(|| {
        standard_heading_fallback(id).map_or_else(|| format!("[missing: {id}]"), str::to_owned)
    })
}

/// Supplies English labels only for the recognized standard heading IDs.
fn standard_heading_fallback(id: &str) -> Option<&'static str> {
    match id {
        "ortho.headings.name" => Some("NAME"),
        "ortho.headings.synopsis" => Some("SYNOPSIS"),
        "ortho.headings.description" => Some("DESCRIPTION"),
        "ortho.headings.options" => Some("OPTIONS"),
        "ortho.headings.environment" => Some("ENVIRONMENT"),
        "ortho.headings.files" => Some("FILES"),
        "ortho.headings.precedence" => Some("PRECEDENCE"),
        "ortho.headings.exit_status" => Some("EXIT STATUS"),
        "ortho.headings.examples" => Some("EXAMPLES"),
        "ortho.headings.see_also" => Some("SEE ALSO"),
        "ortho.headings.commands" => Some("COMMANDS"),
        _ => None,
    }
}

/// Keeps absent optional IDs absent and resolves present IDs using the required-message policy.
fn resolve_optional(localizer: &dyn Localizer, id: Option<&str>) -> Option<String> {
    id.map(|value| resolve_message(localizer, value))
}
