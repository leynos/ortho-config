//! Slot-by-slot integrity assertions over the generated corpus.
//!
//! These assertions hold the half of V2 that equality with the model cannot:
//! they compare each node's original and projected records, so a slot that
//! must survive or must be cleared is checked against the input rather than
//! against the model's output, and a traversal that dropped it while agreeing
//! with the model would still fail here. The comparison is not independent of
//! the model everywhere: whether command examples must be suppressed comes
//! from `model_has_protected`, so a protected-field decision shared by the
//! projection and the model is outside what these assertions can catch. Each
//! group compares one tuple, which keeps every slot in a single comparison
//! that a later edit cannot narrow, and `assert_slots_survive!` expands every
//! group's slot list into that comparison so the check itself has one
//! implementation.

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
    assert_eq!(
        original.fields.len(),
        projected.fields.len(),
        "field count must be preserved"
    );
    for (before, after) in original.fields.iter().zip(&projected.fields) {
        assert_field_integrity(before, after);
    }
    assert_eq!(
        original.subcommands.len(),
        projected.subcommands.len(),
        "projection must preserve the subcommand count"
    );
    for (before, after) in original.subcommands.iter().zip(&projected.subcommands) {
        assert_same_node(before, after, has_protected);
    }
}

/// Compares one group of borrowed slots between a before and an after value.
///
/// Each caller lists its slot paths once; the macro expands them into a single
/// `assert_eq!`, so a later edit cannot narrow a group without changing its
/// list, and the comparison itself has one implementation. Names may be dotted
/// field paths (`sections.discovery`); the two operands must be bindings
/// holding the compared values, not expressions.
macro_rules! assert_slots_survive {
    ($before:expr, $after:expr, $expectation:literal; $($($slot:ident).+),+ $(,)?) => {
        assert_eq!(
            ($( &$before $(.$slot)+, )+),
            ($( &$after $(.$slot)+, )+),
            $expectation
        );
    };
}

/// Asserts the scalar document identity fields survive unchanged.
///
/// The five scalars are compared as one tuple, so there is no way for a later
/// edit to drop one comparison without changing the tuple.
fn assert_node_identity(original: &DocMetadata, projected: &DocMetadata) {
    assert_slots_survive!(
        original, projected,
        "document identity must survive projection";
        ir_version, app_name, bin_name, about_id, synopsis_id
    );
}

/// Asserts the section metadata survives and examples follow the global rule.
fn assert_node_sections(original: &DocMetadata, projected: &DocMetadata, has_protected: bool) {
    assert_slots_survive!(
        original, projected,
        "section metadata must survive projection";
        sections.headings_ids, sections.discovery, sections.precedence,
        sections.links, sections.notes
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
        assert_eq!(
            original.sections.examples, projected.sections.examples,
            "clean-command examples must survive projection"
        );
    }
}

/// Asserts every document-level declaration record survives unchanged.
///
/// `output` is compared here too: it carries capabilities and exit classes
/// rather than field values, so the projection must copy it verbatim. Leaving
/// it out of the tuple would let a traversal that dropped it pass.
fn assert_node_records(original: &DocMetadata, projected: &DocMetadata) {
    assert_slots_survive!(
        original, projected,
        "document records must survive projection";
        windows, behaviour, renderer, output, profiles
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
    assert_slots_survive!(
        before, after, "public descriptor must survive projection";
        name, help_id, long_help_id, required, profile_field
    );
}

/// Asserts the auxiliary records and the structural value shape survive.
///
/// The value type is compared through [`value_shape`] rather than by equality:
/// a protected enum's *members* may be cleared, but its shape must not change.
fn assert_field_records(before: &FieldMetadata, after: &FieldMetadata) {
    assert_slots_survive!(
        before, after, "auxiliary records must survive projection";
        deprecated, env, file, links, notes
    );
    assert_eq!(
        before.value.as_ref().map(value_shape),
        after.value.as_ref().map(value_shape),
        "value shape must survive projection"
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
    assert_slots_survive!(
        before, after, "public CLI spelling must survive projection";
        long, short, value_name, multiple, takes_value, value_optional, hide_in_help
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
        assert_eq!(
            before.possible_values, after.possible_values,
            "ordinary possible values must survive projection"
        );
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
