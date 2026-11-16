//! An independent model of the disclosure projection.
//!
//! This is the differential oracle for V4: it rebuilds a projected document
//! slot by slot instead of cloning and mutating one, so a missed slot in the
//! production traversal and a missed slot in the model would have to be made
//! twice, in two differently shaped implementations, to agree.
//!
//! Correspondence review, one line per modelled slot:
//!
//! - Document: `ir_version`, `app_name`, `bin_name`, `about_id`, `synopsis_id`,
//!   `windows`, `behaviour`, `renderer`, and `profiles` are copied unchanged.
//! - Sections: `headings_ids`, `discovery`, `precedence`, `links`, and `notes`
//!   are copied unchanged; `examples` are cleared for every node when any field
//!   anywhere in the tree is protected, and copied unchanged otherwise.
//! - Field: `name`, `help_id`, `long_help_id`, `required`, `deprecated`, `env`,
//!   `file`, `links`, `notes`, and `profile_field` are copied unchanged. On a
//!   protected field `default` becomes absent, `examples` empty, and
//!   `cli.possible_values` empty; an ordinary field keeps all three.
//! - CLI: `long`, `short`, `value_name`, `multiple`, `takes_value`,
//!   `value_optional`, and `hide_in_help` are copied unchanged. Only
//!   `possible_values` follows the field's classification.
//! - Value type: `String`, `Integer`, `Float`, `Bool`, `Duration`, `Path`,
//!   `IpAddr`, `Hostname`, `Url`, and `Custom` are copied unchanged. `Enum`
//!   loses its `variants` only on a protected field; `List` and `Map` rebuild
//!   their element type with the same rule.

use crate::docs::{CliMetadata, DocMetadata, FieldMetadata, ValueType};

/// Projects a document by rebuilding every slot instead of mutating a clone.
pub(super) fn model_project(metadata: &DocMetadata) -> DocMetadata {
    let has_protected = model_has_protected(metadata);
    model_project_node(metadata, has_protected)
}

/// Reports whether any field anywhere in the tree is protected.
pub(super) fn model_has_protected(metadata: &DocMetadata) -> bool {
    metadata
        .fields
        .iter()
        .any(|field| field.profile_field.is_some())
        || metadata.subcommands.iter().any(model_has_protected)
}

/// Rebuilds one command node and its descendants.
fn model_project_node(metadata: &DocMetadata, has_protected: bool) -> DocMetadata {
    let mut projected = metadata.clone();
    if has_protected {
        projected.sections.examples.clear();
    }
    projected.fields = metadata.fields.iter().map(model_project_field).collect();
    projected.subcommands = metadata
        .subcommands
        .iter()
        .map(|child| model_project_node(child, has_protected))
        .collect();
    projected
}

/// Rebuilds one field, clearing exactly the protected slots.
fn model_project_field(field: &FieldMetadata) -> FieldMetadata {
    let is_protected = field.profile_field.is_some();
    FieldMetadata {
        name: field.name.clone(),
        help_id: field.help_id.clone(),
        long_help_id: field.long_help_id.clone(),
        value: field
            .value
            .as_ref()
            .map(|value| model_value(value, is_protected)),
        default: if is_protected {
            None
        } else {
            field.default.clone()
        },
        required: field.required,
        deprecated: field.deprecated.clone(),
        cli: field.cli.as_ref().map(|cli| model_cli(cli, is_protected)),
        env: field.env.clone(),
        file: field.file.clone(),
        examples: if is_protected {
            Vec::new()
        } else {
            field.examples.clone()
        },
        links: field.links.clone(),
        notes: field.notes.clone(),
        profile_field: field.profile_field,
    }
}

/// Rebuilds CLI metadata, clearing possible values only when protected.
fn model_cli(cli: &CliMetadata, is_protected: bool) -> CliMetadata {
    CliMetadata {
        long: cli.long.clone(),
        short: cli.short,
        value_name: cli.value_name.clone(),
        multiple: cli.multiple,
        takes_value: cli.takes_value,
        value_optional: cli.value_optional,
        possible_values: if is_protected {
            Vec::new()
        } else {
            cli.possible_values.clone()
        },
        hide_in_help: cli.hide_in_help,
    }
}

/// Rebuilds a value type, clearing enum members only when protected.
fn model_value(value: &ValueType, is_protected: bool) -> ValueType {
    match value {
        ValueType::Enum { variants } if is_protected => ValueType::Enum {
            variants: Vec::new(),
        },
        ValueType::List { of } => ValueType::List {
            of: Box::new(model_value(of, is_protected)),
        },
        ValueType::Map { of } => ValueType::Map {
            of: Box::new(model_value(of, is_protected)),
        },
        other => other.clone(),
    }
}

/// Reports whether the corpus contains at least one protected literal, which
/// makes the sentinel-absence assertion non-vacuous.
pub(super) fn has_protected_literal(metadata: &DocMetadata) -> bool {
    metadata.fields.iter().any(|field| {
        field.profile_field.is_some()
            && (field.default.is_some()
                || !field.examples.is_empty()
                || field
                    .cli
                    .as_ref()
                    .is_some_and(|cli| !cli.possible_values.is_empty())
                || field.value.as_ref().is_some_and(value_has_member))
    }) || metadata.subcommands.iter().any(has_protected_literal)
}

/// Reports whether a value type carries any enum member.
pub(super) fn value_has_member(value: &ValueType) -> bool {
    match value {
        ValueType::Enum { variants } => !variants.is_empty(),
        ValueType::List { of } | ValueType::Map { of } => value_has_member(of),
        _ => false,
    }
}
