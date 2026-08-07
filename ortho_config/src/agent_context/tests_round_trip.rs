//! Property coverage for compact agent-context JSON round trips.

use super::*;
use crate::serialize_agent_context;
use proptest::{collection::vec, option, prelude::*};

proptest! {
    #[test]
    fn to_json_always_round_trips(context in any_agent_context()) {
        let json = serialize_agent_context(&context).expect("serialize compact agent context");
        let parsed: AgentContext =
            serde_json::from_str(&json).expect("parse compact agent context");

        prop_assert_eq!(parsed, context);
    }
}

fn any_agent_context() -> impl Strategy<Value = AgentContext> {
    (package_name(), vec(any_agent_command(), 0..4)).prop_map(|(package, commands)| AgentContext {
        schema_version: ORTHO_AGENT_CONTEXT_SCHEMA_VERSION.to_owned(),
        kind: crate::agent_context_kind(&package),
        package,
        commands,
        profiles: SupportDeclaration { supported: false },
        feedback: SupportDeclaration { supported: false },
        policy: AgentPolicy {
            agent_native: PolicyMode::Warn,
        },
        skill_manifests: Vec::new(),
    })
}

fn any_agent_command() -> impl Strategy<Value = AgentCommand> {
    (
        vec(command_segment(), 1..4),
        option::of(summary()),
        option::of(command_segment()),
        vec(any_agent_input(), 0..4),
        vec(output_mode(), 0..4),
        interaction_mode(),
        mutation_effect(),
        // proptest strategies only tuple up to twelve elements, so the two
        // flag-name strategies share one slot.
        (option::of(flag_name()), option::of(flag_name())),
        option::of(async_submission()),
        option::of(delivery_route()),
        option::of(pagination_contract()),
        vec(any_agent_example(), 0..4),
    )
        .prop_map(
            |(
                path,
                summary,
                canonical_verb,
                inputs,
                output_modes,
                interaction_mode,
                mutation_effect,
                (bypass_flag, dry_run_flag),
                async_submission,
                delivery_route,
                pagination,
                examples,
            )| AgentCommand {
                path,
                summary,
                canonical_verb,
                inputs,
                output_modes,
                interaction_mode,
                mutation_effect,
                bypass_flag,
                dry_run_flag,
                async_submission,
                delivery_route,
                pagination,
                examples,
            },
        )
}

fn any_agent_input() -> impl Strategy<Value = AgentInput> {
    (
        command_segment(),
        option::of(command_segment()),
        option::of(value_type()),
        any::<bool>(),
        option::of(summary()),
        vec(summary(), 0..4),
    )
        .prop_map(
            |(name, long, value_type, required, default, enum_values)| AgentInput {
                name,
                long,
                value_type,
                required,
                default,
                enum_values,
            },
        )
}

fn any_agent_example() -> impl Strategy<Value = AgentExample> {
    (summary(), option::of(output_mode())).prop_map(|(command, output_mode)| AgentExample {
        command,
        output_mode,
    })
}

fn async_submission() -> impl Strategy<Value = AsyncSubmission> {
    (async_submission_mode(), option::of(summary()))
        .prop_map(|(mode, noun)| AsyncSubmission { mode, noun })
}

fn async_submission_mode() -> impl Strategy<Value = AsyncSubmissionMode> {
    prop_oneof![
        Just(AsyncSubmissionMode::Inline),
        Just(AsyncSubmissionMode::Submit),
    ]
}

fn delivery_route() -> impl Strategy<Value = DeliveryRoute> {
    (any::<bool>(), option::of(summary()))
        .prop_map(|(supported, target)| DeliveryRoute { supported, target })
}

fn pagination_contract() -> impl Strategy<Value = PaginationContract> {
    (option::of(command_segment()), option::of(command_segment())).prop_map(
        |(limit_input, cursor_input)| PaginationContract {
            limit_input,
            cursor_input,
        },
    )
}

fn output_mode() -> impl Strategy<Value = String> {
    prop_oneof![Just("json".to_owned()), Just("text".to_owned())]
}

fn value_type() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("string".to_owned()),
        Just("bool".to_owned()),
        Just("path".to_owned()),
        Just("enum".to_owned()),
    ]
}

fn interaction_mode() -> impl Strategy<Value = InteractionMode> {
    prop_oneof![
        Just(InteractionMode::Unknown),
        Just(InteractionMode::NonInteractive),
        Just(InteractionMode::Interactive),
    ]
}

fn mutation_effect() -> impl Strategy<Value = MutationEffect> {
    prop_oneof![
        Just(MutationEffect::Unknown),
        Just(MutationEffect::ReadOnly),
        Just(MutationEffect::Write),
        Just(MutationEffect::Delete),
        Just(MutationEffect::Submit),
    ]
}

fn package_name() -> impl Strategy<Value = String> {
    "[A-Za-z0-9_.-]{0,16}"
}

fn command_segment() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,12}"
}

fn flag_name() -> impl Strategy<Value = String> {
    "--[a-z0-9]+(-[a-z0-9]+)*"
}

fn summary() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 .,;-]{0,48}"
}
