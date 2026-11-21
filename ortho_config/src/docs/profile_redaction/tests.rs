//! Unit tests for the profile disclosure projection.

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::json;

use super::test_fixtures::{
    doc, example, field, first_subcommand, first_subcommand_mut, mixed_tree, only_field,
    only_field_mut, second_subcommand,
};
use super::{ProfileFieldExposure, traversal_probe};
use crate::docs::{DefaultValue, DocMetadata, DocProfilesMeta, FieldMetadata, ValueType};

#[rstest]
#[case(ProfileFieldExposure::Secret)]
#[case(ProfileFieldExposure::ReferenceOnly)]
fn protected_field_clears_every_literal_slot(#[case] exposure: ProfileFieldExposure) {
    let mut marked = field("token", Some(exposure));
    marked.value = Some(ValueType::List {
        of: Box::new(ValueType::Map {
            of: Box::new(ValueType::Enum {
                variants: vec!["alpha".into(), "beta".into()],
            }),
        }),
    });
    let original = marked.clone();

    let projected = doc("demo", vec![marked], Vec::new()).redacted_for_output();
    let projected_field = only_field(&projected);

    // The expected field is the original with exactly the value-bearing slots
    // the rule removes; comparing whole fields also proves every public slot
    // and the label survived, without listing them one at a time.
    let mut expected = original.clone();
    expected.default = None;
    expected.examples.clear();
    if let Some(cli) = expected.cli.as_mut() {
        cli.possible_values.clear();
    }
    expected.value = Some(ValueType::List {
        of: Box::new(ValueType::Map {
            of: Box::new(ValueType::Enum {
                variants: Vec::new(),
            }),
        }),
    });

    assert_eq!(
        projected_field, &expected,
        "protected fields keep public slots, clear values"
    );
}

#[test]
fn ordinary_field_keeps_every_literal_slot() {
    let original = field("plain", None);
    let projected = doc("demo", vec![original.clone()], Vec::new()).redacted_for_output();

    assert_eq!(
        projected.fields.len(),
        1,
        "ordinary projection preserves the field count"
    );
    assert_eq!(
        only_field(&projected),
        &original,
        "ordinary fields must survive projection"
    );
}

#[test]
fn projection_is_idempotent_on_a_mixed_tree() {
    let once = mixed_tree().redacted_for_output();
    let twice = once.redacted_for_output();

    assert_eq!(
        once, twice,
        "projection must be idempotent on an already-projected tree"
    );
}

#[test]
fn group_projection_applies_a_sibling_decision_to_every_member() {
    let clean = doc("clean", vec![field("plain", None)], Vec::new());
    let protected = doc(
        "protected",
        vec![field("token", Some(ProfileFieldExposure::Secret))],
        Vec::new(),
    );

    // The clean member's own projection keeps its example, so a group result
    // that removes it cannot be explained by projecting each member alone.
    assert_eq!(
        clean.clone().redacted_for_output().sections.examples.len(),
        1,
        "a clean document projected alone keeps its command examples"
    );

    let members = DocMetadata::redacted_group_for_output(vec![clean, protected]);

    assert_eq!(members.len(), 2, "the group keeps every member");
    let earlier = members.first().expect("the group keeps its first member");
    let later = members.get(1).expect("the group keeps its second member");
    assert_eq!(
        earlier.sections.examples.len(),
        0,
        "a protected later sibling suppresses an earlier member's examples"
    );
    // Comparing labels on both sides of the call keeps this test from passing
    // on an implementation that removes classifications instead of examples.
    let later_field = later
        .fields
        .first()
        .expect("the protected member keeps its field");
    assert_eq!(
        later_field.profile_field,
        Some(ProfileFieldExposure::Secret),
        "the group keeps the protected field's own label"
    );
    assert!(
        later_field.default.is_none(),
        "the protected member loses its own default"
    );
    let earlier_field = earlier
        .fields
        .first()
        .expect("the clean member keeps its field");
    assert_eq!(
        earlier_field.profile_field, None,
        "the group does not invent a classification for a clean member"
    );
    assert!(
        earlier_field.default.is_some(),
        "the group keeps a clean member's ordinary default"
    );
}

#[test]
fn protected_field_suppresses_command_examples_throughout_the_tree() {
    let projected = mixed_tree().redacted_for_output();

    // Command examples are suppressed at every node; field examples are cleared
    // only on the protected field, so the two ordinary fields keep theirs.
    let example_counts = (
        projected.sections.examples.len(),
        first_subcommand(&projected).sections.examples.len(),
        second_subcommand(&projected).sections.examples.len(),
        only_field(&projected).examples.len(),
        only_field(first_subcommand(&projected)).examples.len(),
        only_field(second_subcommand(&projected)).examples.len(),
    );

    assert_eq!(
        example_counts,
        (0, 0, 0, 1, 0, 1),
        "command examples are suppressed tree-wide; only the protected field loses its own"
    );
}

#[test]
fn command_examples_survive_when_no_field_is_protected() {
    let ordinary = doc("demo", vec![field("plain", None)], Vec::new());
    let projected = ordinary.redacted_for_output();

    assert_eq!(
        projected.sections.examples.len(),
        1,
        "clean trees keep their command examples"
    );
}

#[test]
fn documents_differing_only_in_protected_slots_project_equal() {
    let mut first = mixed_tree();
    let mut second = mixed_tree();
    let protected = only_field_mut(first_subcommand_mut(&mut second));
    protected.default = Some(DefaultValue {
        display: "another-secret".into(),
    });
    protected.examples = vec![example("another-example")];
    second.sections.examples = vec![example("unrelated-root-example")];
    if let Some(cli) = only_field_mut(first_subcommand_mut(&mut second))
        .cli
        .as_mut()
    {
        cli.possible_values = vec!["gamma".into()];
    }
    // Both documents keep the same structural value type; only the enum
    // members, a protected value slot, differ.
    only_field_mut(first_subcommand_mut(&mut first)).value = Some(ValueType::Enum {
        variants: vec!["alpha".into()],
    });
    only_field_mut(first_subcommand_mut(&mut second)).value = Some(ValueType::Enum {
        variants: vec!["gamma".into()],
    });
    first.sections.examples.clear();
    second.sections.examples.clear();
    only_field_mut(first_subcommand_mut(&mut first))
        .examples
        .clear();
    only_field_mut(first_subcommand_mut(&mut second))
        .examples
        .clear();

    assert_eq!(
        first.redacted_for_output(),
        second.redacted_for_output(),
        "documents differing only in protected slots must project equal"
    );
}

#[test]
fn document_level_metadata_is_preserved() {
    let mut original = doc("demo", vec![field("plain", None)], Vec::new());
    original.profiles = Some(DocProfilesMeta {
        flag: "profile".into(),
        env_var: "DEMO_PROFILE".into(),
    });
    original.sections.links = vec![crate::docs::Link {
        text_id: None,
        uri: "https://example.test".into(),
    }];

    let projected = original.redacted_for_output();

    assert_eq!(
        (
            &projected.profiles,
            &projected.sections.links,
            &projected.sections.headings_ids,
            &projected.app_name,
        ),
        (
            &original.profiles,
            &original.sections.links,
            &original.sections.headings_ids,
            &original.app_name,
        ),
        "document-level metadata must survive projection"
    );
}

#[rstest]
#[case(ProfileFieldExposure::Secret, "secret")]
#[case(ProfileFieldExposure::ReferenceOnly, "reference_only")]
fn labels_round_trip_through_their_wire_spelling(
    #[case] exposure: ProfileFieldExposure,
    #[case] wire: &str,
) {
    let encoded = serde_json::to_value(exposure).expect("exposure serializes");
    assert_eq!(
        encoded,
        json!(wire),
        "serialization must use the snake_case wire spelling"
    );
    let decoded: ProfileFieldExposure =
        serde_json::from_value(json!(wire)).expect("known label deserializes");
    assert_eq!(
        decoded, exposure,
        "a wire label must deserialize to its classification"
    );
    assert_eq!(
        exposure.as_str(),
        wire,
        "as_str must return the same wire spelling"
    );
}

#[test]
fn unknown_label_fails_deserialization() {
    let result = serde_json::from_value::<ProfileFieldExposure>(json!("public"));
    assert!(result.is_err(), "unknown labels must not become ordinary");
}

#[test]
fn absent_label_is_omitted_from_the_wire() {
    let encoded = serde_json::to_value(field("plain", None)).expect("field serializes");
    assert!(
        encoded.get("profile_field").is_none(),
        "an absent label is omitted from the wire"
    );
    let decoded: FieldMetadata = serde_json::from_value(encoded).expect("field deserializes");
    assert_eq!(
        decoded.profile_field, None,
        "an absent label must decode as unclassified"
    );
}

#[test]
fn redacted_wire_output_carries_the_label_but_no_literals() {
    let projected = doc(
        "demo",
        vec![field("token", Some(ProfileFieldExposure::Secret))],
        Vec::new(),
    )
    .redacted_for_output();
    let encoded = serde_json::to_value(only_field(&projected)).expect("field serializes");

    // `FieldMetadata` has no `skip_serializing_if` on `default`, so an absent
    // default is an explicit null rather than an omitted key.
    let wire_facts = (
        encoded.get("profile_field"),
        encoded
            .get("default")
            .is_some_and(serde_json::Value::is_null),
        encoded
            .get("examples")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty),
        encoded
            .get("cli")
            .and_then(|cli| cli.get("possible_values")),
    );

    assert_eq!(
        wire_facts,
        (Some(&json!("secret")), true, true, Some(&json!([]))),
        "the wire keeps the label and clears every literal"
    );
}

#[test]
fn projection_performs_two_linear_traversals() {
    const DEPTH: usize = 48;
    let mut node = doc("leaf", vec![field("leaf", None)], Vec::new());
    for _ in 1..DEPTH {
        node = doc("node", vec![field("node", None)], vec![node]);
    }

    traversal_probe::reset();
    let projected = node.redacted_for_output();

    // One pass decides whether any protected field exists; one pass projects.
    assert_eq!(
        traversal_probe::visits(),
        2 * DEPTH,
        "one counting pass plus one projection pass"
    );
    assert_eq!(
        projected.subcommands.len(),
        1,
        "projection must preserve the command tree shape"
    );
}
