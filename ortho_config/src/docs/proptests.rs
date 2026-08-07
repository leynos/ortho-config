//! Property coverage for documentation IR behaviour metadata round trips.

use super::ir::{BehaviourMetadata, InteractionKind, MutationKind};
use proptest::{option, prelude::*};

proptest! {
    #[test]
    fn behaviour_metadata_json_round_trips(metadata in any_behaviour_metadata()) {
        let json = serde_json::to_string(&metadata).expect("serialize behaviour metadata");
        let parsed: BehaviourMetadata =
            serde_json::from_str(&json).expect("parse behaviour metadata");

        prop_assert_eq!(parsed, metadata);
    }

    #[test]
    fn behaviour_metadata_value_round_trips(metadata in any_behaviour_metadata()) {
        let value = serde_json::to_value(&metadata).expect("serialize behaviour metadata");
        let parsed: BehaviourMetadata =
            serde_json::from_value(value).expect("parse behaviour metadata");

        prop_assert_eq!(parsed, metadata);
    }
}

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

fn any_interaction_kind() -> impl Strategy<Value = InteractionKind> {
    prop_oneof![
        Just(InteractionKind::NonInteractive),
        Just(InteractionKind::Interactive),
    ]
}

fn any_mutation_kind() -> impl Strategy<Value = MutationKind> {
    prop_oneof![
        Just(MutationKind::ReadOnly),
        Just(MutationKind::Write),
        Just(MutationKind::Delete),
        Just(MutationKind::Submit),
    ]
}

fn flag_name() -> impl Strategy<Value = String> {
    "--[a-z0-9]+(-[a-z0-9]+)*"
}
