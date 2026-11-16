//! Step definitions for the declared structured-output scenarios.
//!
//! The scenarios build a compiled [`AgentContext`] in process and call the
//! public [`check_output`] rule, so they exercise the reusable API that a
//! consumer would call. They deliberately do not go through
//! `--check-agent-native`: that CLI path evaluates configuration only and has
//! no command metadata to lint, so it cannot observe this rule.
//!
//! Each scenario declares one command, checks it in one enforcement mode, and
//! asserts stable machine fields rather than message prose.

use cargo_orthohelp::policy::PolicyMode;
use cargo_orthohelp::policy::PolicySeverity;
use cargo_orthohelp::policy::rules::output::check_output;
use ortho_config::agent_context::{OutputContract, StdoutPurpose};
use ortho_config::{AgentCommand, AgentContext, InteractionMode, MutationEffect, RendererContract};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};

use crate::rstest_bdd_support::behaviour::steps::StepResult;

/// Scenario state for the structured-output rule.
#[derive(Debug, Default, ScenarioState)]
pub struct OutputPolicyState {
    /// The command under test.
    command: Slot<AgentCommand>,
    /// The report produced by the most recent check.
    report: Slot<cargo_orthohelp::policy::PolicyReport>,
}

/// Provides empty state for one structured-output scenario.
#[fixture]
pub fn output_policy_state() -> OutputPolicyState {
    OutputPolicyState::default()
}

/// Builds a contract declaring only the stdout purpose and JSON capability.
const fn contract(supports_json: Option<bool>, stdout: Option<StdoutPurpose>) -> OutputContract {
    OutputContract {
        supports_json,
        stdout,
        stderr_diagnostics: None,
        exit_classes: Vec::new(),
    }
}

/// Builds an inert command carrying the supplied output block.
///
/// The command is declared non-interactive and read-only so no behaviour rule
/// could contribute a finding.
fn declared_command(path: &[&str], output: Option<OutputContract>) -> AgentCommand {
    AgentCommand {
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
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

#[given("a command with stdout purpose \"data\" and undeclared JSON support")]
fn data_with_undeclared_json(output_policy_state: &mut OutputPolicyState) {
    output_policy_state.command.set(declared_command(
        &["fixture"],
        Some(contract(None, Some(StdoutPurpose::Data))),
    ));
}

#[given("a command with stdout purpose \"data\" and JSON support true")]
fn data_with_json_support(output_policy_state: &mut OutputPolicyState) {
    output_policy_state.command.set(declared_command(
        &["fixture"],
        Some(contract(Some(true), Some(StdoutPurpose::Data))),
    ));
}

#[given("a command with stdout purpose \"data\" and JSON support false")]
fn data_without_json_support(output_policy_state: &mut OutputPolicyState) {
    output_policy_state.command.set(declared_command(
        &["fixture"],
        Some(contract(Some(false), Some(StdoutPurpose::Data))),
    ));
}

#[given("a command named \"get\" with no output declaration")]
fn get_without_declaration(output_policy_state: &mut OutputPolicyState) {
    let mut command = declared_command(&["get"], None);
    // A canonical verb is exactly the temptation to infer output; declaring it
    // makes the scenario prove the rule ignores it.
    command.canonical_verb = Some("get".to_owned());
    output_policy_state.command.set(command);
}

#[when("the output policy is checked in \"deny\" mode")]
fn check_in_deny_mode(output_policy_state: &mut OutputPolicyState) -> StepResult<()> {
    check_in_mode(output_policy_state, PolicyMode::Deny)
}

#[when("the output policy is checked in \"warn\" mode")]
fn check_in_warn_mode(output_policy_state: &mut OutputPolicyState) -> StepResult<()> {
    check_in_mode(output_policy_state, PolicyMode::Warn)
}

#[when("the output policy is checked in \"off\" mode")]
fn check_in_off_mode(output_policy_state: &mut OutputPolicyState) -> StepResult<()> {
    check_in_mode(output_policy_state, PolicyMode::Off)
}

/// Runs the public rule over a one-command context and stores its report.
fn check_in_mode(output_policy_state: &mut OutputPolicyState, mode: PolicyMode) -> StepResult<()> {
    let report = output_policy_state
        .command
        .with_ref(|command| {
            let mut context = AgentContext::new("structured-output-scenario");
            context.commands.push(command.clone());
            check_output(&context, mode)
        })
        .ok_or("the scenario should declare a command before checking the policy")?;
    output_policy_state.report.set(report);
    Ok(())
}

#[then("exactly one \"json_support_missing\" finding has severity \"deny\"")]
fn one_deny_finding(output_policy_state: &mut OutputPolicyState) -> StepResult<()> {
    let report = output_policy_state
        .report
        .with_ref(Clone::clone)
        .ok_or("the output policy should have been checked")?;
    assert_eq!(
        report.results.len(),
        1,
        "expected exactly one finding, got {:#?}",
        report.results
    );
    let finding = report
        .results
        .first()
        .ok_or("a report with one result has a first result")?;
    assert_eq!(
        finding.code, "json_support_missing",
        "unexpected finding code"
    );
    assert_eq!(
        finding.rule_id, "agent-native.output.json-missing",
        "unexpected rule identifier"
    );
    assert_eq!(
        finding.severity,
        PolicySeverity::Deny,
        "the finding should be a hard failure in deny mode"
    );
    assert_eq!(
        report.summary.deny, 1,
        "the summary should count the deny finding"
    );
    Ok(())
}

#[then("the output policy report has no findings")]
fn no_findings(output_policy_state: &mut OutputPolicyState) -> StepResult<()> {
    let report = output_policy_state
        .report
        .with_ref(Clone::clone)
        .ok_or("the output policy should have been checked")?;
    assert!(
        report.results.is_empty(),
        "expected no findings, got {:#?}",
        report.results
    );
    assert_eq!(report.summary.total, 0, "the summary total should be zero");
    assert_eq!(report.summary.warn, 0, "the warning count should be zero");
    assert_eq!(report.summary.deny, 0, "the deny count should be zero");
    Ok(())
}
