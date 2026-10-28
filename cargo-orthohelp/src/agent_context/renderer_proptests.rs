//! Generated checks for the renderer half of the agent-context bridge (E3).
//!
//! These are sampled checks, not proofs, and they state their own limits. Two
//! are shaped so the generator cannot satisfy the property for free:
//!
//! - field identifiers are generated from a mixed Unicode alphabet in arbitrary
//!   order, so the exact-order property exercises the bridge's own pass-through
//!   rather than an order the generator already supplied;
//! - the localizer returns a value that visibly differs from the declared
//!   summary, so the "human output changed" property cannot hold vacuously when
//!   no localizer is applied.
//!
//! The unknown-token property covers only the tolerant wire side. The authoring
//! IR rejects unknown tokens instead of normalizing them, which is asserted by
//! example in `ortho_config/tests/docs_ir_renderer.rs`; generating that case
//! here would require constructing values the strict types cannot represent.

use proptest::prelude::*;

use super::bridge_ir_to_agent_context;
use crate::schema::{
    DocMetadata, HeadingIds, RendererField as IrField, RendererFieldLocalization, RendererMetadata,
    RendererRecord as IrRecord, SectionsMetadata,
};
use ortho_config::{
    AgentContext, LocalizationArgs, Localizer, RendererContract, RendererField,
    RendererFieldLocalization as WireLocalization, TogglePolicy,
};

/// Caps every generated family at 256 cases, per the plan's stated bound.
fn bounded() -> ProptestConfig {
    ProptestConfig::with_cases(256)
}

/// Builds a root-only metadata tree carrying the supplied renderer block.
fn root_with(renderer: Option<RendererMetadata>) -> DocMetadata {
    DocMetadata {
        ir_version: crate::schema::ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: "demo".to_owned(),
        bin_name: Some("demo-bin".to_owned()),
        about_id: "root.about".to_owned(),
        synopsis_id: None,
        sections: sections(),
        fields: Vec::new(),
        subcommands: Vec::new(),
        windows: None,
        behaviour: None,
        renderer,
        profiles: None,
    }
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

/// Generates a token no accepted spelling can produce.
///
/// The `future_` prefix is the witness: the tolerant wire enums must normalize
/// it to `unknown` rather than guessing at a nearby known variant.
fn future_token(prefix: &'static str) -> impl Strategy<Value = String> {
    "[a-z0-9]{0,12}".prop_map(move |suffix| format!("{prefix}_future_{suffix}"))
}

/// Generates one non-empty identifier drawn from a mixed Unicode alphabet.
///
/// Lengths run 1–8 characters because the plan bounds identifiers at 1–64; the
/// alphabet mixes ASCII, accented Latin, and non-Latin scripts so a bridge that
/// normalized or escaped identifiers would change the value.
fn unicode_identifier() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!['a', 'Z', '0', '_', '-', 'é', 'ü', '漢', 'α', 'Ω']),
        1..=8,
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

/// Generates an optional support claim plus an optional toggle policy.
fn declared_record(fields: impl Strategy<Value = Vec<IrField>>) -> impl Strategy<Value = IrRecord> {
    (
        prop::option::of(any::<bool>()),
        prop_oneof![
            Just(None),
            Just(Some(crate::schema::TogglePolicy::Never)),
            Just(Some(crate::schema::TogglePolicy::Auto)),
            Just(Some(crate::schema::TogglePolicy::Always)),
        ],
        fields,
    )
        .prop_map(|(supported, colour, declared_fields)| IrRecord {
            supported,
            colour,
            fields: declared_fields,
            ..IrRecord::default()
        })
}

/// Generates a renderer block with independently chosen records.
fn renderer_block() -> impl Strategy<Value = RendererMetadata> {
    (
        prop::option::of(declared_record(ir_fields())),
        prop::option::of(declared_record(ir_fields())),
    )
        .prop_map(|(human, machine)| RendererMetadata { human, machine })
}

/// A localizer whose answers visibly differ from the declared identifiers.
struct RenamingLocalizer;

impl Localizer for RenamingLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        Some(format!("localized summary for {id}"))
    }
}

/// Projects a tree and returns the renderer of its single root command.
fn project_renderer(tree: &DocMetadata) -> RendererContract {
    bridge_ir_to_agent_context(tree, "demo_pkg", None)
        .commands
        .into_iter()
        .next()
        .expect("a root command is always emitted")
        .renderer
}

proptest! {
    #![proptest_config(bounded())]

    /// Declared support survives the projection, and an undeclared role keeps
    /// its documented default rather than inheriting the other role's claim.
    #[test]
    fn generated_support_claims_survive_the_projection(block in renderer_block()) {
        let expected_human = block
            .human
            .as_ref()
            .and_then(|record| record.supported)
            .unwrap_or(true);
        let expected_machine = block
            .machine
            .as_ref()
            .and_then(|record| record.supported)
            .unwrap_or(false);

        let renderer = project_renderer(&root_with(Some(block)));

        prop_assert_eq!(renderer.human.supported, expected_human);
        prop_assert_eq!(renderer.machine.supported, expected_machine);
    }

    /// Every declared field identifier reaches the wire byte-for-byte, in
    /// declaration order, with its localization classification intact.
    #[test]
    fn declared_field_identifiers_survive_unchanged(fields in ir_fields()) {
        let block = RendererMetadata {
            human: Some(IrRecord { fields: fields.clone(), ..IrRecord::default() }),
            machine: None,
        };

        let renderer = project_renderer(&root_with(Some(block)));

        let expected: Vec<RendererField> = fields
            .iter()
            .map(|field| RendererField {
                id: field.id.clone(),
                localization: match field.localization {
                    RendererFieldLocalization::Localized => WireLocalization::Localized,
                    RendererFieldLocalization::NonLocalized => WireLocalization::NonLocalized,
                },
            })
            .collect();
        prop_assert_eq!(renderer.human.fields, expected);
    }

    /// Projecting twice yields identical documents, so nothing is order- or
    /// iteration-dependent.
    #[test]
    fn renderer_projection_is_deterministic(block in renderer_block()) {
        let tree = root_with(Some(block));

        let first = serde_json::to_string(&project_renderer(&tree))
            .expect("the renderer contract should serialize");
        let second = serde_json::to_string(&project_renderer(&tree))
            .expect("the renderer contract should serialize");

        prop_assert_eq!(first, second);
    }
}

proptest! {
    #![proptest_config(bounded())]

    /// A localizer changes human-facing summary text but never field identity.
    ///
    /// The two halves are asserted together because the interesting failure is
    /// a localizer leaking into protocol data: the summary must move, and `id`
    /// must not.
    #[test]
    fn localization_changes_summary_but_not_field_identifiers(fields in ir_fields()) {
        let block = RendererMetadata {
            human: Some(IrRecord { fields: fields.clone(), ..IrRecord::default() }),
            machine: None,
        };
        let tree = root_with(Some(block));

        let plain: AgentContext = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let localized = bridge_ir_to_agent_context(&tree, "demo_pkg", Some(&RenamingLocalizer));

        let root_of = |context: &AgentContext| {
            context
                .commands
                .first()
                .expect("a root command is always emitted")
                .clone()
        };
        let plain_root = root_of(&plain);
        let localized_root = root_of(&localized);
        let plain_summary = plain_root.summary.clone();
        let localized_summary = localized_root.summary.clone();
        prop_assert_ne!(
            plain_summary,
            localized_summary,
            "the localizer must visibly change the human summary"
        );

        let localized_ids: Vec<&str> = localized_root
            .renderer
            .human
            .fields
            .iter()
            .map(|field| field.id.as_str())
            .collect();
        let declared_ids: Vec<&str> = fields.iter().map(|field| field.id.as_str()).collect();
        prop_assert_eq!(localized_ids, declared_ids);
    }
}

proptest! {
    #![proptest_config(bounded())]

    /// A token no known spelling matches normalizes to `unknown` on the wire.
    ///
    /// The authoring IR is strict, so this property lives on the wire enums: it
    /// is the tolerance the agent-context contract promises, and the witness
    /// that a future producer's token cannot fail a strict decode here.
    #[test]
    fn future_tokens_normalize_to_unknown(token in future_token("renderer")) {
        let quoted = serde_json::to_string(&token).expect("a token should serialize");

        prop_assert_eq!(
            serde_json::from_str::<TogglePolicy>(&quoted).expect("toggle policy decode"),
            TogglePolicy::Unknown
        );
        prop_assert_eq!(
            serde_json::from_str::<WireLocalization>(&quoted).expect("localization decode"),
            WireLocalization::Unknown
        );
    }
}
