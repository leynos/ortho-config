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

fn ctx_for_command(command: AgentCommand) -> AgentContext {
    context_with(command)
}

fn codes(report: &crate::policy::PolicyReport) -> Vec<&str> {
    report.results.iter().map(|r| r.code.as_str()).collect()
}

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

#[rstest]
#[case(MutationEffect::Write)]
#[case(MutationEffect::Submit)]
#[case(MutationEffect::ReadOnly)]
fn non_delete_mutations_do_not_trigger_the_destructive_rule(#[case] mutation: MutationEffect) {
    let context = ctx_for_command(command(&["apply"], InteractionMode::Interactive, mutation));
    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(!codes(&report).contains(&"destructive_bypass_missing"));
}

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

#[test]
fn empty_command_list_yields_empty_report() {
    let context = AgentContext::new("empty");
    let report = check_behaviour(&context, PolicyMode::Warn);
    assert!(report.results.is_empty());
}

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
