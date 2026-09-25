//! Unit tests for the behaviour lint rules.

use ortho_config::{AgentCommand, AgentContext, AgentInput, InteractionMode, MutationEffect};
use rstest::rstest;
use serde_json::Value;

use super::behaviour::check_behaviour;
use crate::policy::{PolicyMode, PolicySeverity};

/// Builds an agent context containing one command.
fn context_with(command: AgentCommand) -> AgentContext {
    let mut context = AgentContext::new("fixture");
    context.commands.push(command);
    context
}

/// Builds a minimal command with the given path and declared semantics.
///
/// Every optional field starts empty so a test can mutate exactly the field it
/// exercises; `summary` and `canonical_verb` stay absent because the behaviour
/// rules never read them.
fn command(path: &[&str], interaction: InteractionMode, mutation: MutationEffect) -> AgentCommand {
    AgentCommand {
        path: path.iter().map(|s| (*s).to_owned()).collect(),
        summary: None,
        canonical_verb: None,
        inputs: Vec::new(),
        output_modes: Vec::new(),
        interaction_mode: interaction,
        mutation_effect: mutation,
        bypass_flag: None,
        dry_run_flag: None,
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: Vec::new(),
    }
}

/// Builds an optional agent input whose long flag matches its name.
///
/// The identity between name and long flag is what the bypass-known rule
/// matches against, so a caller testing an unknown bypass supplies an input
/// named differently from the declared flag.
fn input(name: &str, value_type: &str) -> AgentInput {
    AgentInput {
        name: name.to_owned(),
        long: Some(name.to_owned()),
        value_type: Some(value_type.to_owned()),
        required: false,
        default: None,
        enum_values: Vec::new(),
    }
}

/// Wraps one command in a context, as most cases need exactly one.
fn ctx_for_command(command: AgentCommand) -> AgentContext {
    context_with(command)
}

/// Collects the rule codes from a report, in the order the rules emitted them.
fn codes(report: &crate::policy::PolicyReport) -> Vec<&str> {
    report.results.iter().map(|r| r.code.as_str()).collect()
}

/// A fully declared destructive command must satisfy every rule at once.
///
/// This is the pass condition the other cases are contrasted against: the
/// command declares a destructive mutation, an interactive mode, and a bypass
/// flag that matches a real input.
#[test]
fn fully_declared_destructive_tree_yields_empty_report_in_warn_mode() {
    let mut cmd = command(
        &["admin", "purge"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    );
    cmd.bypass_flag = Some("--force".to_owned());
    cmd.inputs.push(input("force", "bool"));
    let context = ctx_for_command(cmd);

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(
        report.results.is_empty(),
        "expected no findings, got {:#?}",
        report.results
    );
}

/// A destructive command with no bypass flag is the primary deny case.
#[test]
fn destructive_without_bypass_triggers_destructive_bypass_missing() {
    let context = ctx_for_command(command(
        &["admin", "prune"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    ));

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(codes(&report).contains(&"destructive_bypass_missing"));
}

/// An interactive command needs an escape hatch even when it mutates nothing.
#[test]
fn interactive_without_bypass_triggers_prompt_bypass_missing() {
    let context = ctx_for_command(command(
        &["interact"],
        InteractionMode::Interactive,
        MutationEffect::ReadOnly,
    ));

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(codes(&report).contains(&"prompt_bypass_missing"));
}

/// A command that trips two rules reports the destructive rule first.
///
/// The ordering is contractual for CLI consumers reading the report, so it is
/// pinned here rather than left to rule registration order.
#[test]
fn interactive_destructive_command_without_bypass_preserves_finding_order() {
    let context = ctx_for_command(command(
        &["admin", "purge"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    ));

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert_eq!(
        codes(&report),
        ["destructive_bypass_missing", "prompt_bypass_missing"]
    );
}

/// A declared bypass naming no real input cannot be passed by an agent.
#[test]
fn declared_bypass_not_matching_an_input_triggers_bypass_flag_unknown() {
    let mut cmd = command(
        &["purge"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    );
    cmd.bypass_flag = Some("--force".to_owned());
    cmd.inputs.push(input("recipient", "string"));
    let context = ctx_for_command(cmd);

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(codes(&report).contains(&"bypass_flag_unknown"));
}

/// A command declaring neither axis produces both undeclared findings.
#[test]
fn undeclared_metadata_produces_interaction_unknown_and_mutation_unknown() {
    let context = ctx_for_command(command(
        &["version"],
        InteractionMode::Unknown,
        MutationEffect::Unknown,
    ));

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(codes(&report).contains(&"interaction_unknown"));
    assert!(codes(&report).contains(&"mutation_unknown"));
}

/// Findings follow command-path order, not the order commands were pushed.
///
/// `zebra` is inserted before `alpha`, so an unsorted walk would emit the
/// findings reversed; the assertion pins the sorted result.
#[test]
fn findings_are_emitted_in_command_path_order() {
    let mut context = AgentContext::new("fixture");
    context.commands.push(command(
        &["zebra"],
        InteractionMode::Unknown,
        MutationEffect::Unknown,
    ));
    context.commands.push(command(
        &["alpha"],
        InteractionMode::Unknown,
        MutationEffect::Unknown,
    ));

    let report = check_behaviour(&context, PolicyMode::Warn);
    let command_paths = report
        .results
        .iter()
        .map(|result| {
            if result.message.contains("`alpha`") {
                "alpha"
            } else if result.message.contains("`zebra`") {
                "zebra"
            } else {
                "unexpected"
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(command_paths, ["alpha", "alpha", "zebra", "zebra"]);
}

/// The undeclared-mutation remedy must teach the whole permitted vocabulary.
///
/// A remedy that omitted a boundary would push authors towards the values it
/// happens to mention, so all four are asserted present.
#[test]
fn undeclared_mutation_remedy_lists_every_supported_boundary() {
    let context = ctx_for_command(command(
        &["apply"],
        InteractionMode::NonInteractive,
        MutationEffect::Unknown,
    ));

    let report = check_behaviour(&context, PolicyMode::Warn);
    let message = report
        .results
        .iter()
        .find(|result| result.code == "mutation_unknown")
        .expect("undeclared mutation should produce a finding")
        .message
        .as_str();

    for mutation in ["read_only", "write", "delete", "submit"] {
        assert!(
            message.contains(mutation),
            "remedy should include `{mutation}`, got {message}"
        );
    }
}

/// A bypass on a read-only command is legitimate and must stay unreported.
///
/// It also exercises the negative case for the bypass-known rule: the declared
/// flag does match an input here, so no finding is the correct result.
#[test]
fn bypass_on_non_destructive_command_produces_no_finding() {
    let mut cmd = command(
        &["list"],
        InteractionMode::Interactive,
        MutationEffect::ReadOnly,
    );
    cmd.bypass_flag = Some("--force".to_owned());
    cmd.inputs.push(input("force", "bool"));
    let context = ctx_for_command(cmd);

    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(
        !codes(&report).contains(&"destructive_bypass_missing"),
        "read-only commands must not trigger the destructive rule"
    );
    assert!(
        codes(&report).is_empty(),
        "expected no findings, got {:#?}",
        report.results
    );
}

/// Only `delete` is destructive; the other boundaries must not be treated so.
///
/// `write` and `submit` change state, so a name-based heuristic could mistake
/// them for destructive; the rule keys off the declared mutation alone.
#[rstest]
#[case(MutationEffect::Write)]
#[case(MutationEffect::Submit)]
#[case(MutationEffect::ReadOnly)]
fn non_delete_mutations_do_not_trigger_the_destructive_rule(#[case] mutation: MutationEffect) {
    let context = ctx_for_command(command(&["apply"], InteractionMode::Interactive, mutation));
    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(!codes(&report).contains(&"destructive_bypass_missing"));
}

/// Declaring a command non-interactive is the approved way to skip a bypass.
///
/// A destructive command that never prompts has nothing to bypass, so the
/// destructive rule must not fire on it.
#[test]
fn non_interactive_destructive_command_without_bypass_is_exempt() {
    let context = ctx_for_command(command(
        &["prune"],
        InteractionMode::NonInteractive,
        MutationEffect::Delete,
    ));
    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(
        !codes(&report).contains(&"destructive_bypass_missing"),
        "declared non-interactive commands are the approved metadata path"
    );
}

/// A package with no commands is vacuously compliant.
#[test]
fn empty_command_list_yields_empty_report() {
    let context = AgentContext::new("empty");
    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(report.results.is_empty());
}

/// `off` suppresses every rule but still reports the mode it was asked for.
///
/// The command below would trip both undeclared rules in an enforcing mode, so
/// an empty result proves the rules were skipped rather than satisfied.
#[test]
fn off_mode_returns_empty_report_without_evaluating_rules() {
    let context = ctx_for_command(command(
        &["version"],
        InteractionMode::Unknown,
        MutationEffect::Unknown,
    ));
    let report = check_behaviour(&context, PolicyMode::Off);
    assert!(report.results.is_empty());
    assert_eq!(report.mode, PolicyMode::Off);
}

/// Every finding carries the severity implied by the mode, with no stragglers.
///
/// The summary counters must agree with the results list, so both are checked:
/// a finding labelled deny must not be counted as a warning.
#[rstest]
#[case::warn(PolicyMode::Warn, PolicySeverity::Warn)]
#[case::deny(PolicyMode::Deny, PolicySeverity::Deny)]
fn policy_mode_assigns_the_expected_severity_to_every_finding(
    #[case] mode: PolicyMode,
    #[case] expected_severity: PolicySeverity,
) {
    let context = ctx_for_command(command(
        &["prune"],
        InteractionMode::Unknown,
        MutationEffect::Unknown,
    ));
    let is_warn = mode == PolicyMode::Warn;
    let report = check_behaviour(&context, mode);
    assert!(
        report
            .results
            .iter()
            .all(|r| r.severity == expected_severity)
    );
    if is_warn {
        assert_eq!(report.summary.warn, report.results.len());
        assert_eq!(report.summary.deny, 0);
    } else {
        assert_eq!(report.summary.deny, report.results.len());
    }
}

/// A finding carries the stable rule id, code, and an actionable message.
///
/// The identifiers are the published contract that suppressions key off, and
/// `location` is asserted absent because behaviour findings address commands
/// rather than source spans.
#[test]
fn findings_carry_expected_rule_and_code_identifiers() {
    let context = ctx_for_command(command(
        &["prune"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    ));
    let report = check_behaviour(&context, PolicyMode::Warn);
    let result = report
        .results
        .iter()
        .find(|r| r.code == "destructive_bypass_missing")
        .expect("destructive finding should exist");
    assert_eq!(result.rule_id, "agent-native.behaviour.destructive-bypass");
    assert_eq!(result.location, None);
    assert!(
        result.message.contains("`admin prune`") || result.message.contains("`prune`"),
        "message should name the command path, got {:?}",
        result.message
    );
    assert!(
        result.message.contains("behaviour(bypass"),
        "message should name the remedy, got {:?}",
        result.message
    );
}

/// The report serializes to one flat JSON document with stable header fields.
///
/// `version` and `mode` are asserted by value because downstream CI parses them
/// to decide how to treat the run.
#[test]
fn report_serializes_as_a_single_json_document() {
    let context = ctx_for_command(command(
        &["prune"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    ));
    let report = check_behaviour(&context, PolicyMode::Warn);
    let json = serde_json::to_string(&report).expect("serialize");
    let value: Value = serde_json::from_str(&json).expect("deserialize report document");
    assert_eq!(value.get("version").and_then(Value::as_str), Some("1"));
    assert_eq!(value.get("mode").and_then(Value::as_str), Some("warn"));
}
