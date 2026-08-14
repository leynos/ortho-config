//! Property tests for the agent-native behaviour lint.
//!
//! These properties hold over arbitrary command vectors:
//!
//! - [`check_behaviour`] is total: every mode over every context returns a
//!   report.
//! - enforcing reports never mix severities: `warn` mode never yields a
//!   `deny`-severity finding, and `off` mode yields no findings at all.
//! - `off` always yields an empty result list.

use proptest::collection::vec;
use proptest::prelude::*;

use ortho_config::{AgentCommand, AgentContext, AgentInput, InteractionMode, MutationEffect};

use super::behaviour::check_behaviour;
use crate::policy::{PolicyMode, PolicySeverity};

/// Strategy over a short lowercase slug with optional digits, hyphens, and
/// underscores (matching CLI-path segment rules without flag dashes).
fn slug() -> impl Strategy<Value = String> {
    (
        1..=15usize,
        prop::char::range('a', 'z'),
        vec(
            prop_oneof![
                prop::char::range('a', 'z'),
                prop::char::range('0', '9'),
                Just('-'),
                Just('_'),
            ],
            0..=14,
        ),
    )
        .prop_map(|(len, first, rest)| {
            let mut segment = String::with_capacity(len + rest.len());
            segment.push(first);
            for ch in rest {
                if segment.len() >= len {
                    break;
                }
                segment.push(ch);
            }
            segment
        })
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

fn input(maybe_long: bool) -> impl Strategy<Value = AgentInput> {
    (slug(), prop::bool::ANY, any::<u8>()).prop_map(move |(name, use_long, seed)| AgentInput {
        long: if maybe_long && use_long {
            Some(name.clone())
        } else {
            None
        },
        value_type: Some(
            if seed.is_multiple_of(2) {
                "string"
            } else {
                "bool"
            }
            .to_owned(),
        ),
        required: seed.is_multiple_of(3),
        default: None,
        enum_values: Vec::new(),
        name,
    })
}

/// Strategy over an arbitrary [`AgentCommand`].
fn command() -> impl Strategy<Value = AgentCommand> {
    (
        vec(slug(), 1..=4),
        interaction_mode(),
        mutation_effect(),
        prop::option::of((Just("--"), slug()).prop_map(|(prefix, name)| format!("{prefix}{name}"))),
        vec(input(true), 0..=3),
    )
        .prop_map(
            |(path, interaction_mode, mutation_effect, bypass_flag, inputs)| AgentCommand {
                path,
                summary: None,
                canonical_verb: None,
                inputs,
                output_modes: Vec::new(),
                interaction_mode,
                mutation_effect,
                bypass_flag,
                dry_run_flag: None,
                async_submission: None,
                delivery_route: None,
                pagination: None,
                examples: Vec::new(),
            },
        )
}

/// Strategy over an arbitrary [`AgentContext`] with a populated command list.
fn context() -> impl Strategy<Value = AgentContext> {
    vec(command(), 0..=6).prop_map(|commands| {
        let mut context = AgentContext::new("proptest-fixture");
        context.commands = commands;
        context
    })
}

proptest! {
    /// `check_behaviour` is total: every mode over every context returns a
    /// report without panicking.
    #[test]
    fn check_behaviour_is_total(
        context in context(),
        mode in prop_oneof![
            Just(PolicyMode::Off),
            Just(PolicyMode::Warn),
            Just(PolicyMode::Deny),
        ],
    ) {
        let requested = mode.clone();
        let report = check_behaviour(&context, mode);
        prop_assert_eq!(report.mode, requested);
        prop_assert_eq!(
            report.summary.total,
            report.results.len(),
            "summary.total must equal the result count"
        );
    }

    /// Warn mode never yields a `deny`-severity finding; off mode yields no
    /// findings at all.
    #[test]
    fn reports_never_contain_deny_in_warn_or_off(
        context in context(),
        mode in prop_oneof![
            Just(PolicyMode::Off),
            Just(PolicyMode::Warn),
        ],
    ) {
        let report = check_behaviour(&context, mode);
        for result in &report.results {
            prop_assert_ne!(
                &result.severity,
                &PolicySeverity::Deny,
                "off and warn modes must never report a deny finding"
            );
        }
        prop_assert_eq!(report.summary.deny, 0);
    }

    /// Off mode always yields an empty result list.
    #[test]
    fn off_mode_always_yields_empty_results(context in context()) {
        let report = check_behaviour(&context, PolicyMode::Off);
        prop_assert!(report.results.is_empty());
        prop_assert_eq!(report.summary.total, 0);
        prop_assert_eq!(report.summary.warn, 0);
        prop_assert_eq!(report.summary.deny, 0);
    }
}
