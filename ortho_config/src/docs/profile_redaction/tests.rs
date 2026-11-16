//! Unit tests for the profile disclosure projection.

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::json;

use super::{ProfileFieldExposure, traversal_probe};
use crate::docs::{
    CliMetadata, DefaultValue, DocMetadata, DocProfilesMeta, Example, FieldMetadata, HeadingIds,
    SectionsMetadata, ValueType,
};

fn heading_ids() -> HeadingIds {
    HeadingIds {
        name: "n".into(),
        synopsis: "s".into(),
        description: "d".into(),
        options: "o".into(),
        environment: "e".into(),
        files: "f".into(),
        precedence: "p".into(),
        exit_status: "x".into(),
        examples: "ex".into(),
        see_also: "sa".into(),
        commands: None,
    }
}

fn sections(examples: Vec<Example>) -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: heading_ids(),
        discovery: None,
        precedence: None,
        examples,
        links: Vec::new(),
        notes: Vec::new(),
    }
}

fn example(code: &str) -> Example {
    Example {
        title_id: None,
        code: code.into(),
        body_id: None,
    }
}

fn cli(possible_values: &[&str]) -> CliMetadata {
    CliMetadata {
        long: Some("token".into()),
        short: None,
        value_name: None,
        multiple: false,
        takes_value: true,
        value_optional: false,
        possible_values: possible_values.iter().map(ToString::to_string).collect(),
        hide_in_help: false,
    }
}

fn field(name: &str, exposure: Option<ProfileFieldExposure>) -> FieldMetadata {
    FieldMetadata {
        name: name.into(),
        help_id: format!("help.{name}"),
        long_help_id: Some(format!("long.{name}")),
        value: Some(ValueType::String),
        default: Some(DefaultValue {
            display: format!("{name}-sentinel"),
        }),
        required: false,
        deprecated: None,
        cli: Some(cli(&["one", "two"])),
        env: None,
        file: None,
        examples: vec![example(&format!("{name}-sentinel-example"))],
        links: Vec::new(),
        notes: Vec::new(),
        profile_field: exposure,
    }
}

fn doc(app: &str, fields: Vec<FieldMetadata>, subcommands: Vec<DocMetadata>) -> DocMetadata {
    DocMetadata {
        ir_version: "2.0".into(),
        app_name: app.into(),
        bin_name: None,
        about_id: format!("{app}.about"),
        synopsis_id: None,
        sections: sections(vec![example(&format!("{app}-example"))]),
        fields,
        subcommands,
        windows: None,
        behaviour: None,
        renderer: None,
        profiles: None,
    }
}

/// A root with command examples, an ordinary field, and a protected child.
fn mixed_tree() -> DocMetadata {
    let child = doc(
        "child",
        vec![field("secret", Some(ProfileFieldExposure::Secret))],
        Vec::new(),
    );
    let sibling = doc("sibling", vec![field("plain", None)], Vec::new());
    let mut root = doc("root", vec![field("ordinary", None)], vec![child, sibling]);
    root.sections.examples = vec![example("root-example"), example("root-example-2")];
    root
}

/// Returns the sole field of a document, which every fixture here declares.
fn only_field(metadata: &DocMetadata) -> &FieldMetadata {
    metadata.fields.first().expect("fixtures declare one field")
}

/// Mutable counterpart of [`only_field`].
fn only_field_mut(metadata: &mut DocMetadata) -> &mut FieldMetadata {
    metadata
        .fields
        .first_mut()
        .expect("fixtures declare one field")
}

/// Returns the first subcommand, which every fixture tree declares.
fn first_subcommand(metadata: &DocMetadata) -> &DocMetadata {
    metadata
        .subcommands
        .first()
        .expect("fixture trees declare two subcommands")
}

/// Mutable counterpart of [`first_subcommand`].
fn first_subcommand_mut(metadata: &mut DocMetadata) -> &mut DocMetadata {
    metadata
        .subcommands
        .first_mut()
        .expect("fixture trees declare two subcommands")
}

/// Returns the second subcommand, which every fixture tree declares.
fn second_subcommand(metadata: &DocMetadata) -> &DocMetadata {
    metadata
        .subcommands
        .get(1)
        .expect("fixture trees declare two subcommands")
}

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

    assert_eq!(projected_field, &expected);
}

#[test]
fn ordinary_field_keeps_every_literal_slot() {
    let original = field("plain", None);
    let projected = doc("demo", vec![original.clone()], Vec::new()).redacted_for_output();

    assert_eq!(projected.fields.len(), 1);
    assert_eq!(only_field(&projected), &original);
}

#[test]
fn projection_is_idempotent_on_a_mixed_tree() {
    let once = mixed_tree().redacted_for_output();
    let twice = once.redacted_for_output();

    assert_eq!(once, twice);
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

    assert_eq!(example_counts, (0, 0, 0, 1, 0, 1));
}

#[test]
fn command_examples_survive_when_no_field_is_protected() {
    let ordinary = doc("demo", vec![field("plain", None)], Vec::new());
    let projected = ordinary.redacted_for_output();

    assert_eq!(projected.sections.examples.len(), 1);
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

    assert_eq!(first.redacted_for_output(), second.redacted_for_output());
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
    assert_eq!(encoded, json!(wire));
    let decoded: ProfileFieldExposure =
        serde_json::from_value(json!(wire)).expect("known label deserializes");
    assert_eq!(decoded, exposure);
    assert_eq!(exposure.as_str(), wire);
}

#[test]
fn unknown_label_fails_deserialization() {
    let result = serde_json::from_value::<ProfileFieldExposure>(json!("public"));
    assert!(result.is_err(), "unknown labels must not become ordinary");
}

#[test]
fn absent_label_is_omitted_from_the_wire() {
    let encoded = serde_json::to_value(field("plain", None)).expect("field serializes");
    assert!(encoded.get("profile_field").is_none());
    let decoded: FieldMetadata = serde_json::from_value(encoded).expect("field deserializes");
    assert_eq!(decoded.profile_field, None);
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
        (Some(&json!("secret")), true, true, Some(&json!([])))
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
    assert_eq!(traversal_probe::visits(), 2 * DEPTH);
    assert_eq!(projected.subcommands.len(), 1);
}
