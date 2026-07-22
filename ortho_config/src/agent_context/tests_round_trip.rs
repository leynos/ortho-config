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
        interaction_mode(),
        mutation_effect(),
    )
        .prop_map(
            |(path, summary, interaction_mode, mutation_effect)| AgentCommand {
                path,
                summary,
                canonical_verb: None,
                inputs: Vec::new(),
                output_modes: vec!["json".to_owned()],
                interaction_mode,
                mutation_effect,
                async_submission: None,
                delivery_route: None,
                pagination: None,
                examples: Vec::new(),
            },
        )
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

fn summary() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 .,;-]{0,48}"
}
