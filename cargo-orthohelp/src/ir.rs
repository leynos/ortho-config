//! Localised IR structures for `cargo-orthohelp`.

use ortho_config::docs::{
    CliMetadata, ConfigDiscoveryMeta, DefaultValue, DocMetadata, EnvMetadata, FileMetadata,
    PrecedenceMeta, SourceKind, ValueType, WindowsMetadata,
};
use ortho_config::{LanguageIdentifier, Localizer};
use serde::Serialize;

/// Localised documentation metadata resolved for a single locale.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedDocMetadata {
    pub ir_version: String,
    pub locale: String,
    pub app_name: String,
    pub bin_name: Option<String>,
    pub about: String,
    pub synopsis: Option<String>,
    pub sections: LocalizedSectionsMetadata,
    pub fields: Vec<LocalizedFieldMetadata>,
    pub subcommands: Vec<LocalizedDocMetadata>,
    pub windows: Option<WindowsMetadata>,
}

/// Section metadata with resolved headings and content.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedSectionsMetadata {
    pub headings: LocalizedHeadings,
    pub discovery: Option<LocalizedConfigDiscoveryMeta>,
    pub precedence: Option<LocalizedPrecedenceMeta>,
    pub examples: Vec<LocalizedExample>,
    pub links: Vec<LocalizedLink>,
    pub notes: Vec<LocalizedNote>,
}

/// Localised heading labels for standard sections.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedHeadings {
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

/// Field metadata with resolved help text.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedFieldMetadata {
    pub name: String,
    pub help: String,
    pub long_help: Option<String>,
    pub value: Option<ValueType>,
    pub default: Option<DefaultValue>,
    pub required: bool,
    pub deprecated: Option<LocalizedDeprecation>,
    pub cli: Option<CliMetadata>,
    pub env: Option<EnvMetadata>,
    pub file: Option<FileMetadata>,
    pub examples: Vec<LocalizedExample>,
    pub links: Vec<LocalizedLink>,
    pub notes: Vec<LocalizedNote>,
}

/// Deprecation metadata with resolved notes.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedDeprecation {
    pub note: String,
}

/// Localised configuration discovery metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedConfigDiscoveryMeta {
    pub formats: Vec<ortho_config::docs::ConfigFormat>,
    pub search_paths: Vec<LocalizedPathPattern>,
    pub override_flag_long: Option<String>,
    pub override_env: Option<String>,
    pub xdg_compliant: bool,
}

/// Localised path pattern metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedPathPattern {
    pub pattern: String,
    pub note: Option<String>,
}

/// Localised source precedence metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedPrecedenceMeta {
    pub order: Vec<SourceKind>,
    pub rationale: Option<String>,
}

/// Localised documentation example snippet.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedExample {
    pub title: Option<String>,
    pub code: String,
    pub body: Option<String>,
}

/// Localised link metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedLink {
    pub text: Option<String>,
    pub uri: String,
}

/// Localised note metadata.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedNote {
    pub text: String,
}

/// Resolves Fluent IDs in `DocMetadata` into localised strings.
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

fn localize_sections(
    sections: &ortho_config::docs::SectionsMetadata,
    localizer: &dyn Localizer,
) -> LocalizedSectionsMetadata {
    LocalizedSectionsMetadata {
        headings: LocalizedHeadings {
            name: resolve_message(localizer, &sections.headings_ids.name),
            synopsis: resolve_message(localizer, &sections.headings_ids.synopsis),
            description: resolve_message(localizer, &sections.headings_ids.description),
            options: resolve_message(localizer, &sections.headings_ids.options),
            environment: resolve_message(localizer, &sections.headings_ids.environment),
            files: resolve_message(localizer, &sections.headings_ids.files),
            precedence: resolve_message(localizer, &sections.headings_ids.precedence),
            exit_status: resolve_message(localizer, &sections.headings_ids.exit_status),
            examples: resolve_message(localizer, &sections.headings_ids.examples),
            see_also: resolve_message(localizer, &sections.headings_ids.see_also),
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

fn localize_field(
    field: &ortho_config::docs::FieldMetadata,
    localizer: &dyn Localizer,
) -> LocalizedFieldMetadata {
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

fn localize_precedence(
    precedence: &PrecedenceMeta,
    localizer: &dyn Localizer,
) -> LocalizedPrecedenceMeta {
    LocalizedPrecedenceMeta {
        order: precedence.order.clone(),
        rationale: resolve_optional(localizer, precedence.rationale_id.as_deref()),
    }
}

fn localize_example(
    example: &ortho_config::docs::Example,
    localizer: &dyn Localizer,
) -> LocalizedExample {
    LocalizedExample {
        title: resolve_optional(localizer, example.title_id.as_deref()),
        code: example.code.clone(),
        body: resolve_optional(localizer, example.body_id.as_deref()),
    }
}

fn localize_link(link: &ortho_config::docs::Link, localizer: &dyn Localizer) -> LocalizedLink {
    LocalizedLink {
        text: resolve_optional(localizer, link.text_id.as_deref()),
        uri: link.uri.clone(),
    }
}

fn localize_note(note: &ortho_config::docs::Note, localizer: &dyn Localizer) -> LocalizedNote {
    LocalizedNote {
        text: resolve_message(localizer, &note.text_id),
    }
}

fn resolve_message(localizer: &dyn Localizer, id: &str) -> String {
    localizer
        .lookup(id, None)
        .unwrap_or_else(|| format!("[missing: {id}]"))
}

fn resolve_optional(localizer: &dyn Localizer, id: Option<&str>) -> Option<String> {
    id.map(|value| resolve_message(localizer, value))
}
