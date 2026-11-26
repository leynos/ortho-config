//! Bounded document generators for the projection properties.
//!
//! The corpus is adversarial by construction: every value slot a protected
//! field carries is filled from the `SECRET-` literal family, so absence in the
//! projected output cannot be satisfied by public text that happens to look
//! sensitive, and the `PUBLIC-` family gives the retention assertions a
//! counterpart that must survive.
//!
//! The generators are bounded to depth four, at most eight children and eight
//! fields per node, and a total of 32 command nodes and 64 fields. A node's
//! remaining budgets are split across its children deterministically, so no
//! generated case is filtered out and every class (empty, ordinary-only,
//! protected-only, mixed) is reachable.
//!
//! The independent model lives in `proptest_model.rs` and the slot assertions
//! in `proptest_assertions.rs`; both consume this corpus.

use proptest::option;
use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

use crate::docs::{
    BehaviourMetadata, CliMetadata, DefaultValue, Deprecation, DocMetadata, DocProfilesMeta,
    EnvMetadata, Example, ExitClass, FieldMetadata, FileMetadata, HeadingIds, InteractionKind,
    Link, MutationKind, Note, OutputMetadata, ProfileFieldExposure, RendererMetadata,
    RendererRecord, SectionsMetadata, StdoutPurpose, TtyPolicy, ValueType, WindowsMetadata,
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
                let records = record_block(&app_name);
                sequence(children).prop_map(move |subcommands| DocMetadata {
                    ir_version: "2.1".to_owned(),
                    app_name: app_name.clone(),
                    bin_name: bin_name.clone(),
                    about_id: format!("{app_name}.about"),
                    synopsis_id: None,
                    sections: sections(examples.clone()),
                    fields: own_fields.clone(),
                    subcommands,
                    windows: records.windows.clone(),
                    behaviour: records.behaviour.clone(),
                    renderer: records.renderer.clone(),
                    output: records.output.clone(),
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
                    positional: None,
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

/// The declaration records every generated node carries.
///
/// These are populated rather than left absent: if every node carried `None`,
/// a projection that silently dropped one of the records would be invisible to
/// the slot assertions. Only the scalar fields vary with the node, which keeps
/// the corpus adversarial for the value slots without adding four more
/// strategy dimensions to every node.
struct RecordBlock {
    windows: Option<WindowsMetadata>,
    behaviour: Option<BehaviourMetadata>,
    renderer: Option<RendererMetadata>,
    output: Option<OutputMetadata>,
}

/// Builds the populated declaration records for one node.
fn record_block(app_name: &str) -> RecordBlock {
    RecordBlock {
        windows: Some(WindowsMetadata {
            module_name: Some(format!("{app_name}-module")),
            export_aliases: vec![app_name.to_owned()],
            include_common_parameters: true,
            split_subcommands_into_functions: true,
            help_info_uri: Some(format!("https://example.test/{app_name}")),
        }),
        behaviour: Some(BehaviourMetadata {
            interaction: Some(InteractionKind::NonInteractive),
            mutation: Some(MutationKind::ReadOnly),
            bypass: Some(format!("{app_name}-bypass")),
            dry_run: Some(format!("{app_name}-dry-run")),
        }),
        renderer: Some(RendererMetadata {
            human: Some(RendererRecord {
                supported: Some(true),
                tty: Some(TtyPolicy::Adapts),
                ..RendererRecord::default()
            }),
            machine: None,
        }),
        output: Some(OutputMetadata {
            supports_json: Some(true),
            stdout: Some(StdoutPurpose::Data),
            stderr_diagnostics: Some(true),
            exit_classes: vec![ExitClass {
                name: format!("{app_name}_ok"),
                meaning: format!("the {app_name} command succeeded"),
            }],
        }),
    }
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
