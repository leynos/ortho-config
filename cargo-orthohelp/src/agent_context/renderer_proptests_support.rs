//! Input builders and comparison helpers for the renderer projection properties.
//!
//! Split from `renderer_proptests` so each file stays beneath the 400-line cap.
//! Everything here is either a generated-input strategy or a fixture the
//! properties compare against; keeping it separate leaves the properties
//! themselves readable as statements about the bridge.

use std::num::NonZeroU16;

use proptest::prelude::*;

use crate::schema::{
    AccessibilityPolicy, CliMetadata, ClosedStdinPolicy, DocMetadata, FieldMetadata, HeadingIds,
    InheritancePolicy, PlainPolicy, RenderWidth, RendererField as IrField,
    RendererFieldLocalization, RendererMetadata, RendererRecord as IrRecord, SectionsMetadata,
    StreamContent, TogglePolicy, TtyPolicy,
};
use ortho_config::{AgentCommand, AgentContext, LocalizationArgs, Localizer, RendererContract};

/// Caps every generated family at 256 cases, per the plan's stated bound.
pub(super) fn bounded() -> ProptestConfig {
    ProptestConfig::with_cases(256)
}

/// Builds the stable section-heading metadata the IR requires.
fn sections() -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: HeadingIds {
            name: "heading.name".to_owned(),
            synopsis: "heading.synopsis".to_owned(),
            description: "heading.description".to_owned(),
            options: "heading.options".to_owned(),
            environment: "heading.environment".to_owned(),
            files: "heading.files".to_owned(),
            precedence: "heading.precedence".to_owned(),
            exit_status: "heading.exit-status".to_owned(),
            examples: "heading.examples".to_owned(),
            see_also: "heading.see-also".to_owned(),
            commands: None,
        },
        discovery: None,
        precedence: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

/// Builds one command node with a declared CLI input and optional children.
///
/// The input is what makes the projection's `inputs` list non-trivial, so the
/// protocol projection comparison covers more than renderer leaves.
pub(super) fn node(
    name: &str,
    renderer: Option<RendererMetadata>,
    children: Vec<DocMetadata>,
) -> DocMetadata {
    DocMetadata {
        ir_version: crate::schema::ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: name.to_owned(),
        bin_name: Some(format!("{name}-bin")),
        about_id: format!("{name}.about"),
        synopsis_id: None,
        sections: sections(),
        fields: vec![field_metadata(name)],
        subcommands: children,
        windows: None,
        behaviour: None,
        renderer,
        profiles: None,
    }
}

/// Builds one flag-bearing field so the node projects a declared input.
fn field_metadata(name: &str) -> FieldMetadata {
    FieldMetadata {
        name: format!("{name}_value"),
        help_id: format!("{name}.help"),
        long_help_id: None,
        value: None,
        default: None,
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some(format!("--{name}-value")),
            short: None,
            value_name: None,
            multiple: false,
            takes_value: true,
            value_optional: false,
            possible_values: Vec::new(),
            hide_in_help: false,
        }),
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

/// Generates a token no accepted spelling can produce.
///
/// The `future_` prefix is the witness: the tolerant wire enums must normalize
/// it to `unknown` rather than guessing at a nearby known variant.
pub(super) fn future_token(prefix: &'static str) -> impl Strategy<Value = String> {
    "[a-z0-9]{0,12}".prop_map(move |suffix| format!("{prefix}_future_{suffix}"))
}

/// Generates one non-empty identifier drawn from a mixed Unicode alphabet.
///
/// Lengths run 1–64 characters, the plan's declared range. The alphabet mixes
/// ASCII, accented Latin, and non-Latin scripts so a bridge that normalized or
/// escaped identifiers would change the value.
fn unicode_identifier() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!['a', 'Z', '0', '_', '-', 'é', 'ü', '漢', 'α', 'Ω']),
        1..=64,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// Generates one declared field with a chosen localization token.
fn ir_field() -> impl Strategy<Value = IrField> {
    (
        unicode_identifier(),
        prop_oneof![
            Just(RendererFieldLocalization::Localized),
            Just(RendererFieldLocalization::NonLocalized),
        ],
    )
        .prop_map(|(id, localization)| IrField { id, localization })
}

/// Generates 0–16 declared fields, deduplicated by identifier.
///
/// Duplicates are removed rather than filtered out after the fact, so the
/// shrinking path does not depend on rejection sampling.
fn ir_fields() -> impl Strategy<Value = Vec<IrField>> {
    prop::collection::vec(ir_field(), 0..=16).prop_map(|fields| {
        let mut seen = std::collections::BTreeSet::new();
        fields
            .into_iter()
            .filter(|field| seen.insert(field.id.clone()))
            .collect()
    })
}

/// Generates an absent-or-declared policy from a fixed variant table.
fn optional_policy<T>(variants: Vec<T>) -> impl Strategy<Value = Option<T>>
where
    T: Clone + std::fmt::Debug + 'static,
{
    prop::option::of(prop::sample::select(variants))
}

/// Generates an absent-or-declared output width, including a fixed count.
fn ir_width() -> impl Strategy<Value = Option<RenderWidth>> {
    prop_oneof![
        Just(None),
        Just(Some(RenderWidth::Terminal)),
        Just(Some(RenderWidth::Unbounded)),
        (1u16..=u16::MAX).prop_map(|columns| Some(RenderWidth::Fixed {
            columns: NonZeroU16::new(columns).expect("the generated range excludes zero"),
        })),
    ]
}

/// Generates an absent-or-declared three-state toggle policy.
fn toggle_leaf() -> impl Strategy<Value = Option<TogglePolicy>> {
    optional_policy(vec![
        TogglePolicy::Never,
        TogglePolicy::Auto,
        TogglePolicy::Always,
    ])
}

/// Generates an absent-or-declared stdout/stderr content category.
fn stream_leaf() -> impl Strategy<Value = Option<StreamContent>> {
    optional_policy(vec![
        StreamContent::None,
        StreamContent::Text,
        StreamContent::Structured,
        StreamContent::Mixed,
    ])
}

/// Generates an absent-or-declared inheritance claim.
fn inheritance_leaf() -> impl Strategy<Value = Option<InheritancePolicy>> {
    optional_policy(vec![
        InheritancePolicy::Allowed,
        InheritancePolicy::Forbidden,
    ])
}

/// Generates one renderer record with support and every policy leaf.
///
/// Both records of a block share this generator, so a generated document can
/// declare every policy family on the human side, the machine side, or both.
/// Each leaf is drawn independently, including the two stream categories and
/// the two inheritance claims, so a case can declare `stdout` without `stderr`
/// and can disagree between progress and subprocess inheritance.
fn declared_record(fields: impl Strategy<Value = Vec<IrField>>) -> impl Strategy<Value = IrRecord> {
    let presentation = (
        prop::option::of(any::<bool>()),
        optional_policy(vec![
            TtyPolicy::Independent,
            TtyPolicy::Adapts,
            TtyPolicy::Requires,
        ]),
        optional_policy(vec![
            ClosedStdinPolicy::Ignored,
            ClosedStdinPolicy::Eof,
            ClosedStdinPolicy::Error,
        ]),
        toggle_leaf(),
        optional_policy(vec![
            AccessibilityPolicy::Unspecified,
            AccessibilityPolicy::TextEquivalent,
        ]),
        toggle_leaf(),
        toggle_leaf(),
        toggle_leaf(),
    );
    let output = (
        optional_policy(vec![
            PlainPolicy::Unsupported,
            PlainPolicy::Available,
            PlainPolicy::Always,
        ]),
        stream_leaf(),
        stream_leaf(),
        ir_width(),
        inheritance_leaf(),
        inheritance_leaf(),
    );
    (presentation, output, fields).prop_map(
        |(
            (supported, tty, closed_stdin, colour, accessibility, emoji, progress, pager),
            (plain, stdout, stderr, width, progress_inheritance, subprocess_inheritance),
            declared_fields,
        )| IrRecord {
            supported,
            tty,
            closed_stdin,
            colour,
            emoji,
            progress,
            pager,
            width,
            accessibility,
            plain,
            stdout,
            stderr,
            progress_inheritance,
            subprocess_inheritance,
            fields: declared_fields,
        },
    )
}

/// Generates a renderer block with independently chosen records.
pub(super) fn renderer_block() -> impl Strategy<Value = RendererMetadata> {
    (
        prop::option::of(declared_record(ir_fields())),
        prop::option::of(declared_record(ir_fields())),
    )
        .prop_map(|(human, machine)| RendererMetadata { human, machine })
}

/// Generates a root command with generated children and independent blocks.
///
/// Children may declare a renderer or none, so the projection covers both a
/// declared nested command and one that keeps the legacy defaults.
pub(super) fn generated_tree() -> impl Strategy<Value = DocMetadata> {
    (
        prop::option::of(renderer_block()),
        prop::collection::vec(
            (unicode_identifier(), prop::option::of(renderer_block())),
            0..=3,
        ),
    )
        .prop_map(|(root_renderer, child_specs)| {
            let child_nodes = child_specs
                .into_iter()
                .map(|(name, renderer)| node(&name, renderer, Vec::new()))
                .collect();
            node("demo", root_renderer, child_nodes)
        })
}

/// Builds a fixed tree with populated renderer and child lists.
///
/// It is the named populated case and the substrate for the negative-control
/// test, so both use the same shape.
pub(super) fn populated_tree() -> DocMetadata {
    let human = IrRecord {
        supported: Some(false),
        colour: Some(TogglePolicy::Never),
        fields: vec![
            IrField {
                id: "message".to_owned(),
                localization: RendererFieldLocalization::Localized,
            },
            IrField {
                id: "error_code".to_owned(),
                localization: RendererFieldLocalization::NonLocalized,
            },
        ],
        ..IrRecord::default()
    };
    let machine = IrRecord {
        supported: Some(true),
        stdout: Some(StreamContent::Structured),
        ..IrRecord::default()
    };
    let declared_child = node(
        "greet",
        Some(RendererMetadata {
            human: Some(IrRecord {
                supported: Some(true),
                tty: Some(TtyPolicy::Adapts),
                ..IrRecord::default()
            }),
            machine: None,
        }),
        Vec::new(),
    );
    node(
        "demo",
        Some(RendererMetadata {
            human: Some(human),
            machine: Some(machine),
        }),
        vec![declared_child, node("version", None, Vec::new())],
    )
}

/// A localizer whose answers visibly differ from the declared identifiers.
pub(super) struct RenamingLocalizer;

impl Localizer for RenamingLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        Some(format!("localized summary for {id}"))
    }
}

/// Copies a projection with every cosmetic summary removed.
///
/// The summary is the only projection content a localizer may change, so the
/// stripped document is exactly the protocol projection both runs must share.
pub(super) fn without_summaries(context: &AgentContext) -> AgentContext {
    let mut stripped = context.clone();
    for command in &mut stripped.commands {
        command.summary = None;
    }
    stripped
}

/// Returns the root command of a projected context.
pub(super) fn root_command(context: &AgentContext) -> AgentCommand {
    context
        .commands
        .first()
        .expect("a root command is always emitted")
        .clone()
}

/// Projects a tree and returns the renderer of its single root command.
pub(super) fn project_renderer(tree: &DocMetadata) -> RendererContract {
    crate::agent_context::bridge_ir_to_agent_context(tree, "demo_pkg", None)
        .commands
        .into_iter()
        .next()
        .expect("a root command is always emitted")
        .renderer
}
