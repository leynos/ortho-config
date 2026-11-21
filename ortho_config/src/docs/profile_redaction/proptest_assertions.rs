//! Slot-by-slot integrity assertions over the generated corpus.
//!
//! These assertions hold the half of V2 that equality with the model cannot:
//! they state, group by group, which slots must survive a projection and which
//! must be cleared, so a production traversal and this model that agreed on the
//! same mistake would still fail here. Each group compares one tuple, which
//! keeps every slot in a single comparison that a later edit cannot narrow.

use crate::docs::{CliMetadata, DocMetadata, Example, FieldMetadata, ValueType};

use super::model::{model_has_protected, value_has_member};

/// Asserts every slot the projection must preserve, and every slot it must
/// clear, at every node of the tree.
pub(super) fn assert_projection_integrity(original: &DocMetadata, projected: &DocMetadata) {
    let has_protected = model_has_protected(original);
    assert_same_node(original, projected, has_protected);
}

/// Asserts integrity for one node and recurses into its children.
fn assert_same_node(original: &DocMetadata, projected: &DocMetadata, has_protected: bool) {
    assert_node_identity(original, projected);
    assert_node_sections(original, projected, has_protected);
    assert_node_records(original, projected);
    assert_eq!(original.fields.len(), projected.fields.len());
    for (before, after) in original.fields.iter().zip(&projected.fields) {
        assert_field_integrity(before, after);
    }
    assert_eq!(original.subcommands.len(), projected.subcommands.len());
    for (before, after) in original.subcommands.iter().zip(&projected.subcommands) {
        assert_same_node(before, after, has_protected);
    }
}

/// Asserts the scalar document identity fields survive unchanged.
///
/// The five scalars are compared as one tuple: a mismatch still names the
/// original and projected identities, and there is no way for a later edit to
/// drop one comparison without changing the tuple.
fn assert_node_identity(original: &DocMetadata, projected: &DocMetadata) {
    assert_eq!(
        (
            &original.ir_version,
            &original.app_name,
            &original.bin_name,
            &original.about_id,
            &original.synopsis_id,
        ),
        (
            &projected.ir_version,
            &projected.app_name,
            &projected.bin_name,
            &projected.about_id,
            &projected.synopsis_id,
        ),
        "document identity must survive projection"
    );
}

/// Asserts the section metadata survives and examples follow the global rule.
fn assert_node_sections(original: &DocMetadata, projected: &DocMetadata, has_protected: bool) {
    assert_eq!(
        (
            &original.sections.headings_ids,
            &original.sections.discovery,
            &original.sections.precedence,
            &original.sections.links,
            &original.sections.notes,
        ),
        (
            &projected.sections.headings_ids,
            &projected.sections.discovery,
            &projected.sections.precedence,
            &projected.sections.links,
            &projected.sections.notes,
        ),
        "section metadata must survive projection"
    );
    assert_example_integrity(original, projected, has_protected);
}

/// Asserts command examples follow the document-wide suppression rule.
fn assert_example_integrity(original: &DocMetadata, projected: &DocMetadata, has_protected: bool) {
    if has_protected {
        assert!(
            projected.sections.examples.is_empty(),
            "command examples are suppressed globally"
        );
    } else {
        assert_eq!(original.sections.examples, projected.sections.examples);
    }
}

/// Asserts every document-level declaration record survives unchanged.
///
/// `output` is compared here too: it carries capabilities and exit classes
/// rather than field values, so the projection must copy it verbatim. Leaving
/// it out of the tuple would let a traversal that dropped it pass.
fn assert_node_records(original: &DocMetadata, projected: &DocMetadata) {
    assert_eq!(
        (
            &original.windows,
            &original.behaviour,
            &original.renderer,
            &original.output,
            &original.profiles,
        ),
        (
            &projected.windows,
            &projected.behaviour,
            &projected.renderer,
            &projected.output,
            &projected.profiles,
        ),
        "document records must survive projection"
    );
}

/// Asserts one field's preserved and cleared slots.
fn assert_field_integrity(before: &FieldMetadata, after: &FieldMetadata) {
    assert_field_identity(before, after);
    assert_field_records(before, after);
    assert_cli_integrity(before, after, before.profile_field.is_some());
    if before.profile_field.is_none() {
        assert_eq!(before, after, "ordinary fields are untouched");
    } else {
        assert_cleared_slots(after);
    }
}

/// Asserts a field's public descriptor survives unchanged.
fn assert_field_identity(before: &FieldMetadata, after: &FieldMetadata) {
    assert_eq!(
        (
            &before.name,
            &before.help_id,
            &before.long_help_id,
            before.required,
            &before.profile_field,
        ),
        (
            &after.name,
            &after.help_id,
            &after.long_help_id,
            after.required,
            &after.profile_field,
        ),
        "public descriptor must survive projection"
    );
}

/// Asserts the auxiliary records and the structural value shape survive.
///
/// The value type is compared through [`value_shape`] rather than by equality:
/// a protected enum's *members* may be cleared, but its shape must not change.
fn assert_field_records(before: &FieldMetadata, after: &FieldMetadata) {
    assert_eq!(
        (
            &before.deprecated,
            &before.env,
            &before.file,
            &before.links,
            &before.notes,
            before.value.as_ref().map(value_shape),
        ),
        (
            &after.deprecated,
            &after.env,
            &after.file,
            &after.links,
            &after.notes,
            after.value.as_ref().map(value_shape),
        ),
        "auxiliary records and value shape must survive projection"
    );
}

/// Asserts a protected field lost every value-bearing slot.
fn assert_cleared_slots(after: &FieldMetadata) {
    assert_eq!(after.default, None, "protected defaults are cleared");
    assert_eq!(
        after.examples,
        [] as [Example; 0],
        "protected examples are cleared"
    );
    if let Some(value) = &after.value {
        assert!(
            !value_has_member(value),
            "protected enum members are cleared"
        );
    }
}

/// Asserts the CLI flags survive and possible values follow the label.
fn assert_cli_integrity(before: &FieldMetadata, after: &FieldMetadata, is_protected: bool) {
    match (&before.cli, &after.cli) {
        (Some(before_cli), Some(after_cli)) => {
            assert_cli_flags(before_cli, after_cli);
            assert_possible_values(before_cli, after_cli, is_protected);
        }
        (None, None) => {}
        _ => panic!("CLI metadata presence must be preserved"),
    }
}

/// Asserts the public CLI spelling and switches survive.
fn assert_cli_flags(before: &CliMetadata, after: &CliMetadata) {
    assert_eq!(
        (
            &before.long,
            before.short,
            &before.value_name,
            before.multiple,
            before.takes_value,
            before.value_optional,
            before.hide_in_help,
        ),
        (
            &after.long,
            after.short,
            &after.value_name,
            after.multiple,
            after.takes_value,
            after.value_optional,
            after.hide_in_help,
        ),
        "public CLI spelling must survive projection"
    );
}

/// Asserts possible values follow the field's classification.
fn assert_possible_values(before: &CliMetadata, after: &CliMetadata, is_protected: bool) {
    if is_protected {
        assert!(
            after.possible_values.is_empty(),
            "protected possible values are cleared"
        );
    } else {
        assert_eq!(before.possible_values, after.possible_values);
    }
}

/// Renders a value type's shape with enum members erased.
fn value_shape(value: &ValueType) -> String {
    match value {
        ValueType::String => "string".to_owned(),
        ValueType::Integer { bits, signed } => format!("integer:{bits}:{signed}"),
        ValueType::Float { bits } => format!("float:{bits}"),
        ValueType::Bool => "bool".to_owned(),
        ValueType::Duration => "duration".to_owned(),
        ValueType::Path => "path".to_owned(),
        ValueType::IpAddr => "ip".to_owned(),
        ValueType::Hostname => "host".to_owned(),
        ValueType::Url => "url".to_owned(),
        ValueType::Enum { .. } => "enum".to_owned(),
        ValueType::List { of } => format!("list<{}>", value_shape(of)),
        ValueType::Map { of } => format!("map<{}>", value_shape(of)),
        ValueType::Custom { name } => format!("custom:{name}"),
    }
}
