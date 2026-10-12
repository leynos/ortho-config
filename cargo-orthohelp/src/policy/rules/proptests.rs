//! Sampled property tests for the agent-native behaviour lint.
//!
//! These are sampled regression checks over generated command vectors, not
//! proofs of completeness. What each one does and does not establish:
//!
//! - [`findings_match_the_rule_specification`] compares the report against an
//!   independent restatement of the rule contract, so a rule that stops firing
//!   fails the property rather than passing it silently. The restatement is
//!   itself written by hand from the specification; if both it and the rules
//!   were wrong in the same way, this property would not notice.
//! - [`sampled_contexts_never_panic_the_check`] is a bounded no-panic check. It
//!   does *not* establish that `check_behaviour` is total over every context,
//!   only that it returned a report for the sampled ones.
//! - [`off_mode_always_yields_empty_results`] pins the suppression contract:
//!   rules that would otherwise fire produce nothing. It asserts on the
//!   externally visible output, not on whether rules were internally skipped.
//!
//! Severity and counter arithmetic are asserted exhaustively by mode in the
//! first property rather than sampled, because they have no input dependence
//! beyond the mode itself.

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

/// Generates every interaction mode, including the undeclared `Unknown`.
///
/// `Unknown` is included so the finding-producing case is *reachable* from the
/// generator. Reachability alone would not make a missing finding detectable;
/// [`expected_codes`] is what supplies the expectation that does.
fn interaction_mode() -> impl Strategy<Value = InteractionMode> {
    prop_oneof![
        Just(InteractionMode::Unknown),
        Just(InteractionMode::NonInteractive),
        Just(InteractionMode::Interactive),
    ]
}

/// Generates every mutation boundary, including the undeclared `Unknown`.
///
/// `Unknown` is included for the same reason as in [`interaction_mode`].
fn mutation_effect() -> impl Strategy<Value = MutationEffect> {
    prop_oneof![
        Just(MutationEffect::Unknown),
        Just(MutationEffect::ReadOnly),
        Just(MutationEffect::Write),
        Just(MutationEffect::Delete),
        Just(MutationEffect::Submit),
    ]
}

/// Builds an input whose declared long flag is exactly `long`.
///
/// The bypass-known rule only ever consults `AgentInput.long`, so the long flag
/// is the whole of an input's relevance here and is set explicitly rather than
/// left to a coin flip.
fn agent_input(name: &str, long: Option<String>) -> AgentInput {
    AgentInput {
        name: name.to_owned(),
        long,
        value_type: Some("string".to_owned()),
        required: false,
        default: None,
        enum_values: Vec::new(),
    }
}

/// Generates a bypass declaration together with inputs whose relationship to
/// it is chosen deliberately, not left to independent chance.
///
/// The four arms cover the relations the bypass rules can distinguish:
///
/// 0. no bypass declared, so only the requirement rules can fire;
/// 1. a bypass matching a declared input's long flag;
/// 2. a bypass matching nothing declared;
/// 3. a bypass whose name appears only on an input with *no* long flag, which
///    the rule's `long`-only lookup must not treat as a match.
fn bypass_and_inputs() -> impl Strategy<Value = (Option<String>, Vec<AgentInput>)> {
    (slug(), 0..4u8, vec(slug(), 0..=2)).prop_map(|(name, arm, others)| {
        let unrelated = others
            .into_iter()
            .map(|other| agent_input(&other, Some(other.clone())))
            .collect::<Vec<_>>();
        match arm {
            0 => (None, unrelated),
            1 => {
                let mut inputs = unrelated;
                inputs.push(agent_input(&name, Some(name.clone())));
                (Some(format!("--{name}")), inputs)
            }
            2 => (Some(format!("--{name}")), unrelated),
            _ => {
                let mut inputs = unrelated;
                inputs.push(agent_input(&name, None));
                (Some(format!("--{name}")), inputs)
            }
        }
    })
}

/// Strategy over an arbitrary [`AgentCommand`].
fn command() -> impl Strategy<Value = AgentCommand> {
    (
        vec(slug(), 1..=4),
        interaction_mode(),
        mutation_effect(),
        bypass_and_inputs(),
    )
        .prop_map(
            |(path, interaction_mode, mutation_effect, (bypass_flag, inputs))| AgentCommand {
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

/// The finding codes the rules are specified to emit for one command.
///
/// This restates the contract from ADR-010 and the `behaviour` module
/// documentation: a bypass present suppresses *both* requirement findings; a
/// destructive command means `MutationEffect::Delete` with an interaction other
/// than `NonInteractive`; the bypass-known check compares the declaration's
/// name (leading `--` removed) against declared long flags only; and undeclared
/// interaction and mutation are independent findings. Codes come back in the
/// order the report emits them.
fn expected_codes(command: &AgentCommand) -> Vec<&'static str> {
    let mut codes = Vec::new();
    if command.bypass_flag.is_none() {
        if command.mutation_effect == MutationEffect::Delete
            && command.interaction_mode != InteractionMode::NonInteractive
        {
            codes.push("destructive_bypass_missing");
        }
        if command.interaction_mode == InteractionMode::Interactive {
            codes.push("prompt_bypass_missing");
        }
    }
    if let Some(bypass) = command.bypass_flag.as_deref() {
        let declared = bypass.strip_prefix("--").unwrap_or(bypass);
        let is_known = command
            .inputs
            .iter()
            .filter_map(|input| input.long.as_deref())
            .any(|long| long == declared);
        if !is_known {
            codes.push("bypass_flag_unknown");
        }
    }
    if command.interaction_mode == InteractionMode::Unknown {
        codes.push("interaction_unknown");
    }
    if command.mutation_effect == MutationEffect::Unknown {
        codes.push("mutation_unknown");
    }
    codes
}

/// Sorted finding codes for a whole context, derived independently of the
/// report under test.
fn expected_code_multiset(context: &AgentContext) -> Vec<String> {
    let mut codes = context
        .commands
        .iter()
        .flat_map(expected_codes)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    codes.sort();
    codes
}

proptest! {
    /// Every finding the rules are specified to emit appears, and no others do.
    ///
    /// Findings are compared as a sorted multiset of codes rather than in
    /// emission order: the report orders by command path using an unstable
    /// sort, so two commands sharing a path have no defined relative order,
    /// while the multiset is fully defined either way.
    #[test]
    fn findings_match_the_rule_specification(
        context in context(),
        mode in prop_oneof![Just(PolicyMode::Warn), Just(PolicyMode::Deny)],
    ) {
        let requested = mode;
        let report = check_behaviour(&context, mode);

        let mut actual = report
            .results
            .iter()
            .map(|result| result.code.clone())
            .collect::<Vec<_>>();
        actual.sort();

        prop_assert_eq!(actual, expected_code_multiset(&context));
        prop_assert_eq!(report.mode, requested);

        let expected_severity = match requested {
            PolicyMode::Warn => PolicySeverity::Warn,
            _ => PolicySeverity::Deny,
        };
        for result in &report.results {
            prop_assert_eq!(&result.severity, &expected_severity);
        }

        prop_assert_eq!(report.summary.off, 0);
        prop_assert_eq!(report.summary.total, report.results.len());
        let (expected_warn, expected_deny) = match requested {
            PolicyMode::Warn => (report.results.len(), 0),
            _ => (0, report.results.len()),
        };
        prop_assert_eq!(report.summary.warn, expected_warn);
        prop_assert_eq!(report.summary.deny, expected_deny);
    }

    /// Sampled no-panic check: every mode over every generated context returns
    /// a report carrying the requested mode and a self-consistent total.
    #[test]
    fn sampled_contexts_never_panic_the_check(
        context in context(),
        mode in prop_oneof![
            Just(PolicyMode::Off),
            Just(PolicyMode::Warn),
            Just(PolicyMode::Deny),
        ],
    ) {
        let requested = mode;
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
