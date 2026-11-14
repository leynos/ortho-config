//! Sampled property tests for the structured-output lint.
//!
//! These are bounded regression checks over generated command vectors, not
//! proofs of completeness. What each one does and does not establish:
//!
//! - [`findings_match_the_specification`] compares the report against an
//!   independent restatement of the rule, so a rule that stops firing or fires
//!   for a non-data purpose fails rather than passing silently. If both the
//!   rule and the restatement were wrong in the same way, the property would
//!   not notice.
//! - [`reordering_distinct_paths_preserves_the_report`] pins the sort
//!   contract: the report is a function of the command set, not of the order
//!   the generator happened to produce.
//! - [`findings_are_additive`] checks that one traversal has no cross-command
//!   state, so splitting an input cannot change any count.
//! - [`checking_does_not_mutate_the_context`] and
//!   [`equal_paths_are_not_deduplicated`] cover side-effect freedom and the
//!   per-occurrence reporting rule.
//!
//! The mode dimension is exhaustive in the first property: every generated
//! context is checked in all three modes, so severity and counter arithmetic
//! are covered for each mode instead of being split across runs.

use proptest::collection::vec;
use proptest::prelude::*;

use ortho_config::agent_context::{ExitClass, OutputContract, StdoutPurpose};
use ortho_config::{AgentCommand, AgentContext, InteractionMode, MutationEffect, RendererContract};

use super::output::check_output;
use super::proptests::slug;
use crate::policy::{PolicyMode, PolicySeverity};

/// Generates every stdout purpose, including the undeclared and tolerant
/// unknown states.
fn stdout_purpose() -> impl Strategy<Value = Option<StdoutPurpose>> {
    prop_oneof![
        Just(None),
        Just(Some(StdoutPurpose::Data)),
        Just(Some(StdoutPurpose::Path)),
        Just(Some(StdoutPurpose::Summary)),
        Just(Some(StdoutPurpose::None)),
        Just(Some(StdoutPurpose::Unknown)),
    ]
}

/// Generates a contract whose every leaf is independently reachable.
fn output_contract() -> impl Strategy<Value = OutputContract> {
    (
        any::<Option<bool>>(),
        stdout_purpose(),
        any::<Option<bool>>(),
        0..=3usize,
    )
        .prop_map(
            |(supports_json, stdout, stderr_diagnostics, class_count)| OutputContract {
                supports_json,
                stdout,
                stderr_diagnostics,
                exit_classes: (0..class_count)
                    .map(|index| ExitClass {
                        name: format!("class{index}"),
                        meaning: "generated meaning".to_owned(),
                    })
                    .collect(),
            },
        )
}

/// Generates an output block that may also be entirely absent.
fn output_block() -> impl Strategy<Value = Option<OutputContract>> {
    prop_oneof![Just(None), output_contract().prop_map(Some)]
}

/// Builds a command from its two varying parts.
///
/// The behaviour declaration stays inert so the output rule is the only
/// possible source of findings in these properties.
fn build_command(path: Vec<String>, output: Option<OutputContract>) -> AgentCommand {
    AgentCommand {
        path,
        summary: None,
        canonical_verb: None,
        inputs: Vec::new(),
        output_modes: Vec::new(),
        interaction_mode: InteractionMode::NonInteractive,
        mutation_effect: MutationEffect::ReadOnly,
        bypass_flag: None,
        dry_run_flag: None,
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: Vec::new(),
        renderer: RendererContract::default(),
        output,
    }
}

/// Generates an arbitrary command.
fn command() -> impl Strategy<Value = AgentCommand> {
    (vec(slug(), 1..=3), output_block()).prop_map(|(path, output)| build_command(path, output))
}

/// Builds a context from the supplied commands.
fn context(commands: Vec<AgentCommand>) -> AgentContext {
    let mut context = AgentContext::new("output-proptest");
    context.commands = commands;
    context
}

/// The finding code the specification requires for one command, if any.
///
/// This restates the rule from the policy specification rather than calling
/// the production predicate.
fn expected_code(command: &AgentCommand) -> Option<&'static str> {
    let contract = command.output.as_ref()?;
    let is_data = contract.stdout == Some(StdoutPurpose::Data);
    let has_json = contract.supports_json == Some(true);
    if is_data && !has_json {
        Some("json_support_missing")
    } else {
        None
    }
}

/// Sorted codes for a whole context, derived independently of the report.
fn expected_codes(commands: &[AgentCommand], mode: PolicyMode) -> Vec<String> {
    if mode == PolicyMode::Off {
        return Vec::new();
    }
    let mut codes = commands
        .iter()
        .filter_map(expected_code)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    codes.sort();
    codes
}

/// A command that always trips the rule.
fn firing_command() -> AgentCommand {
    build_command(
        vec!["dup".to_owned()],
        Some(OutputContract {
            supports_json: Some(false),
            stdout: Some(StdoutPurpose::Data),
            stderr_diagnostics: None,
            exit_classes: Vec::new(),
        }),
    )
}

proptest! {
    /// Every specified finding appears, and no others do, in every mode.
    #[test]
    fn findings_match_the_specification(
        commands in vec(command(), 0..=64),
        mode in prop_oneof![
            Just(PolicyMode::Off),
            Just(PolicyMode::Warn),
            Just(PolicyMode::Deny),
        ],
    ) {
        let report = check_output(&context(commands.clone()), mode);

        let mut actual = report
            .results
            .iter()
            .map(|result| result.code.clone())
            .collect::<Vec<_>>();
        actual.sort();
        prop_assert_eq!(actual, expected_codes(&commands, mode));
        prop_assert_eq!(report.mode, mode);

        let expected_severity = if mode == PolicyMode::Warn {
            PolicySeverity::Warn
        } else {
            PolicySeverity::Deny
        };
        for result in &report.results {
            prop_assert_eq!(&result.severity, &expected_severity);
            prop_assert_eq!(result.rule_id.as_str(), "agent-native.output.json-missing");
            prop_assert!(result.location.is_none());
        }

        prop_assert_eq!(report.summary.off, 0);
        prop_assert_eq!(report.summary.total, report.results.len());
        let (expected_warn, expected_deny) = if mode == PolicyMode::Warn {
            (report.results.len(), 0)
        } else {
            (0, report.results.len())
        };
        prop_assert_eq!(report.summary.warn, expected_warn);
        prop_assert_eq!(report.summary.deny, expected_deny);
    }

    /// Reversing the input order cannot change a report over distinct paths.
    ///
    /// Distinctness is forced by prefixing an index, so the property is not
    /// undermined by path ties the report is allowed to order either way.
    #[test]
    fn reordering_distinct_paths_preserves_the_report(commands in vec(command(), 0..=16)) {
        let mut uniquely_named = commands;
        for (index, entry) in uniquely_named.iter_mut().enumerate() {
            entry.path.insert(0, format!("c{index}"));
        }

        let forward = check_output(&context(uniquely_named.clone()), PolicyMode::Deny);
        let mut reversed_commands = uniquely_named;
        reversed_commands.reverse();
        let backward = check_output(&context(reversed_commands), PolicyMode::Deny);

        prop_assert_eq!(forward.results, backward.results);
        prop_assert_eq!(forward.summary, backward.summary);
    }

    /// Splitting an input cannot change the total or severity counts.
    #[test]
    fn findings_are_additive(
        commands in vec(command(), 0..=32),
        split in 0..=32usize,
    ) {
        let boundary = split.min(commands.len());
        let (left, right) = commands.split_at(boundary);

        let whole = check_output(&context(commands.clone()), PolicyMode::Deny);
        let left_report = check_output(&context(left.to_vec()), PolicyMode::Deny);
        let right_report = check_output(&context(right.to_vec()), PolicyMode::Deny);

        prop_assert_eq!(
            whole.results.len(),
            left_report.results.len() + right_report.results.len()
        );
        prop_assert_eq!(
            whole.summary.deny,
            left_report.summary.deny + right_report.summary.deny
        );
        prop_assert_eq!(whole.summary.total, whole.results.len());
    }

    /// The checker reads the context without changing it.
    #[test]
    fn checking_does_not_mutate_the_context(commands in vec(command(), 0..=32)) {
        let context = context(commands);
        let snapshot = context.clone();
        let checked = check_output(&context, PolicyMode::Deny);

        prop_assert_eq!(context, snapshot);
        prop_assert_eq!(checked.summary.total, checked.results.len());
    }

    /// Equal paths are reported once per occurrence, never deduplicated.
    #[test]
    fn equal_paths_are_not_deduplicated(count in 0..=8usize) {
        let commands = (0..count).map(|_| firing_command()).collect::<Vec<_>>();
        let report = check_output(&context(commands), PolicyMode::Deny);

        prop_assert_eq!(report.results.len(), count);
        prop_assert_eq!(report.summary.deny, count);
    }
}
