//! Bounded document generators and an independent projection model.
//!
//! The model is the differential oracle for V4: it rebuilds a projected
//! document slot by slot instead of cloning and mutating one, so a missed slot
//! in the production traversal and a missed slot in the model would have to be
//! made twice, in two differently shaped implementations, to agree.
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
//!
//! The generators are bounded to depth four, at most eight children and eight
//! fields per node, and a total of 32 command nodes and 64 fields. A node's
//! remaining budgets are split across its children deterministically, so no
//! generated case is filtered out and every class (empty, ordinary-only,
//! protected-only, mixed) is reachable.

use proptest::option;
use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

use crate::docs::{
    CliMetadata, DefaultValue, Deprecation, DocMetadata, DocProfilesMeta, EnvMetadata, Example,
    FieldMetadata, FileMetadata, HeadingIds, Link, Note, ProfileFieldExposure, SectionsMetadata,
    ValueType,
};

/// Maximum generated command nesting.
pub(super) const MAX_DEPTH: usize = 4;
/// Maximum generated command nodes per case.
pub(super) const MAX_NODES: usize = 32;
/// Maximum generated fields per case.
pub(super) const MAX_FIELDS: usize = 64;
/// Maximum children or fields generated for one node.
const MAX_BRANCH: usize = 8;
/// Prefix carried by every protected literal in the corpus.
pub(super) const PROTECTED_SENTINEL: &str = "SECRET-";

/// Generates a bounded documentation tree.
pub(super) fn any_doc() -> impl Strategy<Value = DocMetadata> {
    doc_with_budget(MAX_DEPTH, MAX_NODES, MAX_FIELDS)
}

/// Generates one node and spends its node and field budgets on its children.
fn doc_with_budget(depth: usize, nodes: usize, fields: usize) -> BoxedStrategy<DocMetadata> {
    let max_fields = fields.min(MAX_BRANCH);
    let max_children = if depth == 0 {
        0
    } else {
        MAX_BRANCH.min(nodes.saturating_sub(1))
    };
    (
        public_text(),
        option::of(public_text()),
        prop::collection::vec(any_field(), 0..=max_fields),
        0..=max_children,
        prop::collection::vec(any_example(public_text().boxed()), 0..=4),
        option::of(any_profiles()),
    )
        .prop_flat_map(
            move |(app_name, bin_name, own_fields, child_count, examples, profiles)| {
                let child_nodes = split_budget(nodes - 1, child_count);
                let child_fields = split_budget(fields - own_fields.len(), child_count);
                let children = child_nodes
                    .into_iter()
                    .zip(child_fields)
                    .map(|(node_budget, field_budget)| {
                        doc_with_budget(depth - 1, node_budget, field_budget)
                    })
                    .collect();
                sequence(children).prop_map(move |subcommands| DocMetadata {
                    ir_version: "2.0".to_owned(),
                    app_name: app_name.clone(),
                    bin_name: bin_name.clone(),
                    about_id: format!("{app_name}.about"),
                    synopsis_id: None,
                    sections: sections(examples.clone()),
                    fields: own_fields.clone(),
                    subcommands,
                    windows: None,
                    behaviour: None,
                    renderer: None,
                    profiles: profiles.clone(),
                })
            },
        )
        .boxed()
}

/// Combines a dynamically sized list of strategies into one list strategy.
fn sequence(strategies: Vec<BoxedStrategy<DocMetadata>>) -> BoxedStrategy<Vec<DocMetadata>> {
    strategies
        .into_iter()
        .fold(Just(Vec::new()).boxed(), |combined, next| {
            (combined, next)
                .prop_map(|(mut docs, doc)| {
                    docs.push(doc);
                    docs
                })
                .boxed()
        })
}

/// Splits `total` across `parts`, giving the remainder to the earliest parts.
///
/// Callers that need a minimum of one per part ensure `total >= parts` first;
/// the field budget has no such minimum.
fn split_budget(total: usize, parts: usize) -> Vec<usize> {
    if parts == 0 {
        return Vec::new();
    }
    let base = total
        .checked_div(parts)
        .expect("the early return above excludes zero parts");
    let remainder = total
        .checked_rem(parts)
        .expect("the early return above excludes zero parts");
    (0..parts)
        .map(|index| base + usize::from(index < remainder))
        .collect()
}

/// Generates a field, choosing sentinel literals from its classification.
fn any_field() -> impl Strategy<Value = FieldMetadata> {
    any_exposure().prop_flat_map(|exposure| {
        let literals: BoxedStrategy<String> = if exposure.is_some() {
            secret_text().boxed()
        } else {
            public_text().boxed()
        };
        let surfaces = (
            option::of(any_cli(literals.clone())),
            option::of(public_text().prop_map(|var_name| EnvMetadata { var_name })),
            option::of(public_text().prop_map(|key_path| FileMetadata { key_path })),
            prop::collection::vec(any_example(literals.clone()), 0..=4),
            prop::collection::vec(any_link(), 0..=2),
            prop::collection::vec(public_text().prop_map(|text_id| Note { text_id }), 0..=2),
        );
        (
            Just(exposure),
            field_name(),
            option::of(public_text()),
            option::of(any_value(exposure.is_some())),
            option::of(literals.prop_map(|display| DefaultValue { display })),
            any::<bool>(),
            option::of(public_text().prop_map(|note_id| Deprecation { note_id })),
            surfaces,
        )
            .prop_map(
                |(
                    profile_field,
                    name,
                    long_help_id,
                    value,
                    default,
                    required,
                    deprecated,
                    (cli, env, file, examples, links, notes),
                )| {
                    let help_id = format!("help.{name}");
                    FieldMetadata {
                        name,
                        help_id,
                        long_help_id,
                        value,
                        default,
                        required,
                        deprecated,
                        cli,
                        env,
                        file,
                        examples,
                        links,
                        notes,
                        profile_field,
                    }
                },
            )
    })
}

/// Generates both classifications and the ordinary case.
fn any_exposure() -> impl Strategy<Value = Option<ProfileFieldExposure>> {
    prop_oneof![
        Just(None),
        Just(Some(ProfileFieldExposure::Secret)),
        Just(Some(ProfileFieldExposure::ReferenceOnly)),
    ]
}

/// Generates field names, including repeated leaf names across commands.
fn field_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("token".to_owned()),
        Just("value".to_owned()),
        public_text()
    ]
}

/// Generates CLI metadata whose possible values follow the field label.
fn any_cli(literals: BoxedStrategy<String>) -> impl Strategy<Value = CliMetadata> {
    (
        option::of(public_text()),
        option::of(any::<char>()),
        option::of(public_text()),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(literals, 0..=4),
        any::<bool>(),
    )
        .prop_map(
            |(long, short, value_name, multiple, takes_value, value_optional, values, hidden)| {
                CliMetadata {
                    long,
                    short,
                    value_name,
                    multiple,
                    takes_value,
                    value_optional,
                    possible_values: values,
                    hide_in_help: hidden,
                }
            },
        )
}

/// Generates a value type whose enum members follow the field label.
fn any_value(is_protected: bool) -> BoxedStrategy<ValueType> {
    let literals: BoxedStrategy<String> = if is_protected {
        secret_text().boxed()
    } else {
        public_text().boxed()
    };
    let leaf = prop_oneof![
        Just(ValueType::String),
        Just(ValueType::Bool),
        Just(ValueType::Duration),
        Just(ValueType::Path),
        Just(ValueType::IpAddr),
        Just(ValueType::Hostname),
        Just(ValueType::Url),
        (0u8..=64, any::<bool>()).prop_map(|(bits, signed)| ValueType::Integer { bits, signed }),
        (0u8..=64).prop_map(|bits| ValueType::Float { bits }),
        public_text().prop_map(|name| ValueType::Custom { name }),
        prop::collection::vec(literals, 0..=4).prop_map(|variants| ValueType::Enum { variants }),
    ]
    .boxed();
    leaf.prop_recursive(3, 8, 2, |inner| {
        prop_oneof![
            inner
                .clone()
                .prop_map(|of| ValueType::List { of: Box::new(of) }),
            inner.prop_map(|of| ValueType::Map { of: Box::new(of) }),
        ]
    })
    .boxed()
}

/// Generates a field-level example whose code follows the field label.
fn any_example(literals: BoxedStrategy<String>) -> impl Strategy<Value = Example> {
    (
        option::of(public_text()),
        literals,
        option::of(public_text()),
    )
        .prop_map(|(title_id, code, body_id)| Example {
            title_id,
            code,
            body_id,
        })
}

/// Generates a related link.
fn any_link() -> impl Strategy<Value = Link> {
    (
        option::of(public_text()),
        public_text().prop_map(|suffix| format!("https://example.test/{suffix}")),
    )
        .prop_map(|(text_id, uri)| Link { text_id, uri })
}

/// Generates profile selection metadata.
fn any_profiles() -> impl Strategy<Value = DocProfilesMeta> {
    (public_text(), public_text()).prop_map(|(flag, env_var)| DocProfilesMeta { flag, env_var })
}

/// Public literal strategy.
fn public_text() -> impl Strategy<Value = String> {
    "PUBLIC-[a-z]{0,32}"
}

/// Protected literal strategy.
fn secret_text() -> impl Strategy<Value = String> {
    "SECRET-[a-z]{0,32}"
}

/// Fixed heading identifiers; the projection must not touch them.
fn heading_ids() -> HeadingIds {
    HeadingIds {
        name: "name".to_owned(),
        synopsis: "synopsis".to_owned(),
        description: "description".to_owned(),
        options: "options".to_owned(),
        environment: "environment".to_owned(),
        files: "files".to_owned(),
        precedence: "precedence".to_owned(),
        exit_status: "exit_status".to_owned(),
        examples: "examples".to_owned(),
        see_also: "see_also".to_owned(),
        commands: None,
    }
}

/// Builds a sections block carrying the generated command examples.
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
fn value_has_member(value: &ValueType) -> bool {
    match value {
        ValueType::Enum { variants } => !variants.is_empty(),
        ValueType::List { of } | ValueType::Map { of } => value_has_member(of),
        _ => false,
    }
}

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
fn assert_node_identity(original: &DocMetadata, projected: &DocMetadata) {
    assert_eq!(original.ir_version, projected.ir_version);
    assert_eq!(original.app_name, projected.app_name);
    assert_eq!(original.bin_name, projected.bin_name);
    assert_eq!(original.about_id, projected.about_id);
    assert_eq!(original.synopsis_id, projected.synopsis_id);
}

/// Asserts the section metadata survives and examples follow the global rule.
fn assert_node_sections(original: &DocMetadata, projected: &DocMetadata, has_protected: bool) {
    assert_eq!(
        original.sections.headings_ids,
        projected.sections.headings_ids
    );
    assert_eq!(original.sections.discovery, projected.sections.discovery);
    assert_eq!(original.sections.precedence, projected.sections.precedence);
    assert_eq!(original.sections.links, projected.sections.links);
    assert_eq!(original.sections.notes, projected.sections.notes);
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

/// Asserts the remaining document-level records survive unchanged.
fn assert_node_records(original: &DocMetadata, projected: &DocMetadata) {
    assert_eq!(original.windows, projected.windows);
    assert_eq!(original.behaviour, projected.behaviour);
    assert_eq!(original.renderer, projected.renderer);
    assert_eq!(original.profiles, projected.profiles);
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
    assert_eq!(before.name, after.name);
    assert_eq!(before.help_id, after.help_id);
    assert_eq!(before.long_help_id, after.long_help_id);
    assert_eq!(before.required, after.required);
    assert_eq!(before.profile_field, after.profile_field);
}

/// Asserts the auxiliary records and the structural value shape survive.
fn assert_field_records(before: &FieldMetadata, after: &FieldMetadata) {
    assert_eq!(before.deprecated, after.deprecated);
    assert_eq!(before.env, after.env);
    assert_eq!(before.file, after.file);
    assert_eq!(before.links, after.links);
    assert_eq!(before.notes, after.notes);
    assert_eq!(
        before.value.as_ref().map(value_shape),
        after.value.as_ref().map(value_shape)
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
    assert_eq!(before.long, after.long);
    assert_eq!(before.short, after.short);
    assert_eq!(before.value_name, after.value_name);
    assert_eq!(before.multiple, after.multiple);
    assert_eq!(before.takes_value, after.takes_value);
    assert_eq!(before.value_optional, after.value_optional);
    assert_eq!(before.hide_in_help, after.hide_in_help);
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
