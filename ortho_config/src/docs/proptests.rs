//! Property coverage for documentation IR behaviour metadata round trips.

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
    /// The two routes exercise different serde entry points, so a `rename` or
    /// `skip_serializing_if` mistake can break one without the other.
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
/// declaration in between.
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
