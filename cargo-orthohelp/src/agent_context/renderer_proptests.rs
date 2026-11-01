//! Generated checks for the renderer half of the agent-context bridge (E3).
//!
//! These are sampled checks, not proofs, and they state their own limits.
//! Three are shaped so the generator cannot satisfy the property for free:
//!
//! - identifiers are generated from a mixed Unicode alphabet in arbitrary
//!   order, so the projection properties exercise the bridge's own pass-through
//!   rather than an order the generator already supplied;
//! - the localizer returns a value that visibly differs from the declared
//!   identifiers, so the "summary moved" assertion cannot hold vacuously when
//!   no localizer is applied;
//! - the negative-control test below mutates a projected field identifier and
//!   proves the projection comparison notices, so the localization property is
//!   not silently blind to protocol data.
//!
//! Class coverage: both renderer records, declared support values and policy
//! leaves, nested command paths, declared CLI inputs, and 0–16 output fields
//! with 1–64 character identifiers. Generated cases are bounded at 256 per
//! property, and each failure prints proptest's own seed and minimized case.
//!
//! The unknown-token property covers only the tolerant wire side. The authoring
//! IR rejects unknown tokens instead of normalizing them, which is asserted by
//! example in `ortho_config/tests/docs_ir_renderer.rs`; generating that case
//! here would require constructing values the strict types cannot represent.

#[path = "renderer_proptests_support.rs"]
mod support;

use proptest::prelude::*;
use rstest::rstest;

use super::bridge_ir_to_agent_context;
use crate::schema::{
    AccessibilityPolicy, ClosedStdinPolicy, DocMetadata, InheritancePolicy, PlainPolicy,
    RenderWidth, RendererMetadata, RendererRecord, StreamContent, TogglePolicy, TtyPolicy,
};
use ortho_config::{
    AgentContext, RendererFieldLocalization as WireLocalization, TogglePolicy as WireTogglePolicy,
};
use support::{
    RenamingLocalizer, bounded, future_token, generated_tree, node, populated_tree,
    project_renderer, renderer_block, root_command, without_summaries,
};

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

        let renderer = project_renderer(&node("demo", Some(block), Vec::new()));

        prop_assert_eq!(renderer.human.supported, expected_human);
        prop_assert_eq!(renderer.machine.supported, expected_machine);
    }

    /// A localizer changes only cosmetic summaries, never protocol data.
    ///
    /// The whole document is compared, not just the renderer block: paths,
    /// inputs, and every declared policy must survive, and the summaries must
    /// genuinely move or the equality would be vacuous.
    #[test]
    fn localization_preserves_the_protocol_projection(tree in generated_tree()) {
        let plain = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let localized = bridge_ir_to_agent_context(&tree, "demo_pkg", Some(&RenamingLocalizer));

        let summaries_moved = plain
            .commands
            .iter()
            .zip(&localized.commands)
            .any(|(before, after)| before.summary != after.summary);
        prop_assert!(summaries_moved, "the localizer must visibly change a summary");
        prop_assert_eq!(without_summaries(&plain), without_summaries(&localized));
    }

    /// A projection survives serialization and re-reading without loss.
    #[test]
    fn projection_round_trips_through_the_wire(tree in generated_tree()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let encoded = serde_json::to_value(&context).expect("a context should serialize");
        let decoded: ortho_config::AgentContext =
            serde_json::from_value(encoded).expect("a context should round-trip");

        prop_assert_eq!(decoded, context);
    }

    /// A token no known spelling matches normalizes to `unknown` on the wire.
    ///
    /// The authoring IR is strict, so this property lives on the wire enums: it
    /// is the tolerance the agent-context contract promises, and the witness
    /// that a future producer's token cannot fail a strict decode here.
    #[test]
    fn future_tokens_normalize_to_unknown(token in future_token("renderer")) {
        let quoted = serde_json::to_string(&token).expect("a token should serialize");

        prop_assert_eq!(
            serde_json::from_str::<WireTogglePolicy>(&quoted).expect("toggle policy decode"),
            WireTogglePolicy::Unknown
        );
        prop_assert_eq!(
            serde_json::from_str::<WireLocalization>(&quoted).expect("localization decode"),
            WireLocalization::Unknown
        );
    }
}

/// Named cases for the empty and populated projection shapes.
///
/// The absent block proves both support defaults and empty lists; the populated
/// block proves both explicit support values, declared policies, nested paths,
/// inputs, and output fields together.
#[rstest]
#[case::empty_lists(node("demo", None, Vec::new()), (true, false), 0, 0)]
#[case::populated_lists(populated_tree(), (false, true), 2, 2)]
fn named_projection_cases(
    #[case] tree: DocMetadata,
    #[case] root_support: (bool, bool),
    #[case] expected_children: usize,
    #[case] expected_fields: usize,
) {
    let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
    let root = root_command(&context);

    assert_eq!(
        (
            root.renderer.human.supported,
            root.renderer.machine.supported
        ),
        root_support
    );
    assert_eq!(context.commands.len(), 1 + expected_children);
    assert_eq!(root.inputs.len(), 1, "the root declares one CLI input");
    assert_eq!(root.renderer.human.fields.len(), expected_fields);
}

/// Returns the root command's first projected human field identifier.
fn first_human_field_id(context: &AgentContext) -> String {
    root_command(context)
        .renderer
        .human
        .fields
        .first()
        .map(|field| field.id.clone())
        .expect("the populated tree declares a human field")
}

/// Every declared policy leaf reaches the projection.
///
/// The generated property would stay green if the generator stopped producing
/// a leaf, because both sides of its comparison would lose the same data. This
/// pins one record that declares every leaf and asserts each one survives, so
/// the generator's class coverage is observable rather than assumed.
#[test]
fn every_declared_leaf_reaches_the_projection() {
    let record = RendererRecord {
        supported: Some(true),
        tty: Some(TtyPolicy::Adapts),
        closed_stdin: Some(ClosedStdinPolicy::Eof),
        colour: Some(TogglePolicy::Auto),
        emoji: Some(TogglePolicy::Never),
        progress: Some(TogglePolicy::Always),
        pager: Some(TogglePolicy::Auto),
        width: Some(RenderWidth::Terminal),
        accessibility: Some(AccessibilityPolicy::TextEquivalent),
        plain: Some(PlainPolicy::Available),
        stdout: Some(StreamContent::Text),
        stderr: Some(StreamContent::Structured),
        progress_inheritance: Some(InheritancePolicy::Allowed),
        subprocess_inheritance: Some(InheritancePolicy::Forbidden),
        fields: Vec::new(),
    };
    let block = RendererMetadata {
        human: Some(record),
        machine: None,
    };

    let projected = project_renderer(&node("demo", Some(block), Vec::new())).human;

    let leaves = [
        projected.supported,
        projected.tty.is_some(),
        projected.closed_stdin.is_some(),
        projected.colour.is_some(),
        projected.emoji.is_some(),
        projected.progress.is_some(),
        projected.pager.is_some(),
        projected.width.is_some(),
        projected.accessibility.is_some(),
        projected.plain.is_some(),
        projected.stdout.is_some(),
        projected.stderr.is_some(),
        projected.progress_inheritance.is_some(),
        projected.subprocess_inheritance.is_some(),
    ];
    assert!(
        leaves.iter().all(|declared| *declared),
        "every declared leaf must reach the projection: {leaves:?}"
    );
}

/// The projection comparison detects a localized field identifier.
///
/// This is the standing negative control for the localization property: it
/// performs the mutation that property must catch — replacing a protocol field
/// identifier with localized prose — and asserts the comparison notices. Without
/// it, a comparison that ignored `renderer.fields` would still pass.
#[test]
fn projection_comparison_detects_a_localized_field_identifier() {
    let tree = populated_tree();
    let plain = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
    let mut mutated = bridge_ir_to_agent_context(&tree, "demo_pkg", Some(&RenamingLocalizer));

    let field = mutated
        .commands
        .first_mut()
        .and_then(|command| command.renderer.human.fields.first_mut())
        .expect("the populated tree declares human fields");
    field.id = "localized summary for message".to_owned();

    assert_ne!(
        without_summaries(&plain),
        without_summaries(&mutated),
        "a localized field identifier must fail the protocol projection comparison"
    );
    assert_eq!(
        first_human_field_id(&plain),
        "message",
        "the unmutated projection keeps the declared identifier"
    );
    assert_eq!(
        first_human_field_id(&mutated),
        "localized summary for message"
    );
    let mut restored = without_summaries(&mutated);
    let restored_field = restored
        .commands
        .first_mut()
        .and_then(|command| command.renderer.human.fields.first_mut())
        .expect("the populated tree declares human fields");
    restored_field.id = "message".to_owned();

    assert_eq!(
        restored,
        without_summaries(&plain),
        "the field identifier is the only protocol difference the mutation introduced"
    );
}
