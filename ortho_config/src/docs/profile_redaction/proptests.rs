//! Property coverage for the disclosure projection (V1, V2, V4).
//!
//! Five properties exercise the same bounded corpus:
//!
//! - V1, noninterference: a variant that differs only in protected slots and in
//!   command examples projects to the same document. Verifying that the variant
//!   really differs keeps the property non-vacuous.
//! - V4, correspondence: the production projection equals an independently
//!   rebuilt model over every generated tree, including nested list, map, and
//!   enum value types.
//! - V2, idempotence: projecting an already projected document changes nothing.
//! - V2, integrity: labels, identifiers, CLI flags, document metadata, ordinary
//!   field slots, and command topology survive; protected slots are cleared;
//!   command examples are suppressed exactly when a protected field exists.
//! - Wire absence: the serialized projection carries no protected sentinel,
//!   and the corpus is shown to contain one before projection.
//!
//! The corpus prefixes protected literals with `SECRET-` and public literals
//! with `PUBLIC-`, so absence in the output cannot be satisfied by public text
//! that happens to equal a secret. Generator bounds and the slot-by-slot
//! model correspondence are documented in the support module.

use proptest::prelude::*;

use crate::docs::{DefaultValue, DocMetadata, Example, FieldMetadata, ValueType};

#[path = "proptests_support.rs"]
mod support;
use support::*;

/// Builds a document that differs from `base` only in protected value slots
/// and, when one exists, every command-level example.
fn low_variant(base: &DocMetadata) -> DocMetadata {
    let mut variant = base.clone();
    if !support::model_has_protected(base) {
        return variant;
    }
    rewrite_node(&mut variant);
    variant
}

/// Rewrites protected values and command examples across one subtree.
fn rewrite_node(node: &mut DocMetadata) {
    for example in &mut node.sections.examples {
        example.code = "VARIANT-command-example".to_owned();
    }
    for field in &mut node.fields {
        rewrite_field(field);
    }
    for child in &mut node.subcommands {
        rewrite_node(child);
    }
}

/// Replaces every value slot of a protected field with a distinct sentinel.
fn rewrite_field(field: &mut FieldMetadata) {
    if field.profile_field.is_none() {
        return;
    }
    field.default = Some(DefaultValue {
        display: "VARIANT-default".to_owned(),
    });
    field.examples = vec![Example {
        title_id: None,
        code: "VARIANT-example".to_owned(),
        body_id: None,
    }];
    if let Some(cli) = field.cli.as_mut() {
        cli.possible_values = vec!["VARIANT-possible-value".to_owned()];
    }
    if let Some(value) = field.value.as_mut() {
        rewrite_value(value);
    }
}

/// Replaces a value type's enum members while keeping its shape.
fn rewrite_value(value: &mut ValueType) {
    match value {
        ValueType::Enum { variants } => *variants = vec!["VARIANT-member".to_owned()],
        ValueType::List { of } | ValueType::Map { of } => rewrite_value(of),
        _ => {}
    }
}

proptest! {
    /// V1: low-equivalent documents are indistinguishable after projection.
    #[test]
    fn low_equivalent_documents_project_equal(base in any_doc()) {
        let variant = low_variant(&base);
        if support::model_has_protected(&base) {
            prop_assert_ne!(&variant, &base, "the variant must differ somewhere");
            prop_assert_eq!(base.redacted_for_output(), variant.redacted_for_output());
        } else {
            prop_assert_eq!(&base, &variant);
            prop_assert_eq!(base.redacted_for_output(), base);
        }
    }

    /// V4: the production projection agrees with the independent model.
    #[test]
    fn projection_matches_independent_model(doc in any_doc()) {
        prop_assert_eq!(doc.redacted_for_output(), model_project(&doc));
    }

    /// V2: projection is idempotent.
    #[test]
    fn projection_is_idempotent(doc in any_doc()) {
        let once = doc.redacted_for_output();
        prop_assert_eq!(once.redacted_for_output(), once);
    }

    /// V2: preserved slots survive and cleared slots are cleared.
    #[test]
    fn projection_preserves_and_clears_exactly(doc in any_doc()) {
        let projected = doc.redacted_for_output();
        assert_projection_integrity(&doc, &projected);
    }

    /// Wire: no protected sentinel survives serialization, and the corpus
    /// contains at least one protected sentinel before projection.
    #[test]
    fn projected_wire_carries_no_protected_literal(doc in any_doc()) {
        let projected = doc.redacted_for_output();
        let encoded = serde_json::to_string(&projected).expect("projected metadata serializes");
        prop_assert!(
            !encoded.contains(PROTECTED_SENTINEL),
            "protected literal reached the wire: {encoded}"
        );
        if has_protected_literal(&doc) {
            let original = serde_json::to_string(&doc).expect("metadata serializes");
            prop_assert!(
                original.contains(PROTECTED_SENTINEL),
                "sentinel corpus was vacuous"
            );
        }
    }
}
