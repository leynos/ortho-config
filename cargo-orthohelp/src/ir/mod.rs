//! Localized IR structures for `cargo-orthohelp`.

use ortho_config::{LanguageIdentifier, Localizer};

mod resolvers;
mod types;

use resolvers::{resolve_heading, resolve_message, resolve_optional};

pub use types::{
    LocalizedConfigDiscoveryMeta, LocalizedDeprecation, LocalizedDocMetadata, LocalizedExample,
    LocalizedFieldMetadata, LocalizedHeadings, LocalizedLink, LocalizedNote, LocalizedPathPattern,
    LocalizedPrecedenceMeta, LocalizedSectionsMetadata,
};

use crate::schema::{
    ConfigDiscoveryMeta, DocMetadata, Example, FieldMetadata, Link, Note, PrecedenceMeta,
    SectionsMetadata,
};

/// Resolves Fluent IDs in `DocMetadata` into localized strings.
///
/// The input is projected through [`DocMetadata::redacted_for_output`] before
/// any slot is copied. This function is a public conversion boundary in its
/// own right: a caller that hands in derive-generated or deserialized
/// metadata directly must not be able to publish a protected value merely by
/// skipping the CLI's earlier projection. Projecting twice is harmless
/// because the projection is idempotent.
///
/// # Examples
///
/// ```rust,no_run
/// use cargo_orthohelp::ir::localize_doc;
/// use ortho_config::{LanguageIdentifier, NoOpLocalizer};
///
/// # fn demo(metadata: &ortho_config::docs::DocMetadata) {
/// let locale: LanguageIdentifier = "en-US".parse().expect("valid locale");
/// let localized = localize_doc(metadata, &locale, &NoOpLocalizer::new());
/// assert_eq!(
///     localized.app_name,
///     metadata.app_name,
///     "localization must preserve the application name"
/// );
/// # }
/// ```
pub fn localize_doc(
    metadata: &DocMetadata,
    locale: &LanguageIdentifier,
    localizer: &dyn Localizer,
) -> LocalizedDocMetadata {
    localize_projected_doc(&metadata.redacted_for_output(), locale, localizer)
}

/// Resolves one already-projected document and its descendants.
///
/// Recursion stays private so the public entry point projects the whole tree
/// exactly once. The command-example rule is decided from the entire document,
/// so a nested call to [`localize_doc`] would clone and re-scan every subtree.
fn localize_projected_doc(
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
            .map(|command| localize_projected_doc(command, locale, localizer))
            .collect(),
        windows: metadata.windows.clone(),
        renderer: metadata.renderer.clone(),
        output: metadata.output.clone(),
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

#[cfg(test)]
mod tests;
