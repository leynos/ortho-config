//! Property coverage for documentation IR behaviour metadata round trips.
//!
//! Both properties are **value-preservation** checks: they establish that
//! decoding the encoding of a generated value yields that same value. They are
//! deliberately *not* described as independent wire-contract checks. A
//! symmetric change to the wire names would preserve both round trips while
//! changing the published representation, and adding
//! `skip_serializing_if = "Option::is_none"` would preserve both while turning
//! explicit nulls into omitted keys. The independent expected-representation
//! assertion lives in `ortho_config/tests/docs_ir_behaviour.rs`, which pins the
//! published JSON directly.
//!
//! The properties are also not a check that every combination was *exercised*.
//! The four optional fields make all sixteen declared/absent combinations
//! reachable from the generator; that is a statement about the input space, not
//! about any particular run.

use super::ir::{BehaviourMetadata, InteractionKind, MutationKind};
use proptest::{option, prelude::*};

proptest! {
    /// Serialising to a JSON string and parsing back preserves every field.
    #[test]
    fn behaviour_metadata_json_round_trips(metadata in any_behaviour_metadata()) {
        let json = serde_json::to_string(&metadata).expect("serialize behaviour metadata");
        let parsed: BehaviourMetadata =
            serde_json::from_str(&json).expect("parse behaviour metadata");

        prop_assert_eq!(parsed, metadata);
    }

    /// The same round trip holds through the untyped `Value` representation.
    ///
    /// The two routes enter serde differently — through a string versus through
    /// an already-built `Value` — so each can fail where the other passes. That
    /// gives complementary value-preservation coverage; it does not make them
    /// independent checks of the wire representation, for the reasons in the
    /// module documentation.
    #[test]
    fn behaviour_metadata_value_round_trips(metadata in any_behaviour_metadata()) {
        let value = serde_json::to_value(&metadata).expect("serialize behaviour metadata");
        let parsed: BehaviourMetadata =
            serde_json::from_value(value).expect("parse behaviour metadata");

        prop_assert_eq!(parsed, metadata);
    }
}

/// Generates behaviour blocks covering every declared/absent combination.
///
/// Each of the four fields is independently optional, so the strategy reaches
/// the fully undeclared case, the fully declared case, and every partial
/// declaration in between. Those sixteen combinations are *reachable*; the
/// generator does not prove that a given run sampled each of them.
///
/// The generator's domain is deliberately wider than what the derive accepts.
/// It can pair `NonInteractive` with a bypass, which is a perfectly
/// constructible serde value but a declaration the derive parser rejects as a
/// contradiction. That breadth is intended: these properties test serde
/// fidelity for the type as declared, not the attribute grammar, which the
/// derive enforces at compile time.
///
/// An absent field serializes as an explicit `null` here, because the IR
/// declares no `skip_serializing_if`. The `"unknown"` state is an agent-context
/// concept: the bridge maps an absent interaction or mutation onto it, and the
/// IR never encodes `None` as `"unknown"`.
fn any_behaviour_metadata() -> impl Strategy<Value = BehaviourMetadata> {
    (
        option::of(any_interaction_kind()),
        option::of(any_mutation_kind()),
        option::of(flag_name()),
        option::of(flag_name()),
    )
        .prop_map(
            |(interaction, mutation, bypass, dry_run)| BehaviourMetadata {
                interaction,
                mutation,
                bypass,
                dry_run,
            },
        )
}

/// Generates both declared interaction kinds.
///
/// `Unknown` is deliberately absent: it is the wire representation of an
/// undeclared field, produced by `None` rather than by a value.
fn any_interaction_kind() -> impl Strategy<Value = InteractionKind> {
    prop_oneof![
        Just(InteractionKind::NonInteractive),
        Just(InteractionKind::Interactive),
    ]
}

/// Generates all four declared mutation boundaries.
///
/// `Unknown` is omitted for the same reason as in [`any_interaction_kind`].
fn any_mutation_kind() -> impl Strategy<Value = MutationKind> {
    prop_oneof![
        Just(MutationKind::ReadOnly),
        Just(MutationKind::Write),
        Just(MutationKind::Delete),
        Just(MutationKind::Submit),
    ]
}

/// Generates flag names drawn only from the pinned `--[a-z0-9]+(-[a-z0-9]+)*`
/// grammar.
///
/// Constraining generation to the accepted language keeps the round-trip
/// properties honest: they assert serde fidelity, not grammar validation,
/// which the derive macro already enforces at compile time.
fn flag_name() -> impl Strategy<Value = String> {
    "--[a-z0-9]+(-[a-z0-9]+)*"
}
