//! Tests for the report a behaviour check hands back.
//!
//! These cases pin the output contract rather than rule decisions: the stable
//! identifiers a suppression keys off, the severity accounting in `summary`,
//! the advertised vocabulary, and the remedy text a reader is expected to act
//! on. Rule-by-rule decision behaviour lives in the sibling `tests` module.

use ortho_config::{InteractionMode, MutationEffect};
use rstest::rstest;
use serde_json::Value;

use super::super::behaviour::check_behaviour;
use super::*;
use crate::policy::{PolicyMode, PolicySeverity, Vocabulary};

/// A package with no commands is vacuously compliant.
#[test]
fn empty_command_list_yields_empty_report() {
    let context = ortho_config::AgentContext::new("empty");
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

/// A behaviour report is self-describing: it advertises the canonical
/// vocabulary, exactly as `policy::evaluate` does.
///
/// The published `guide-check-report` example promises a populated
/// `vocabulary` block for this rule set, so an empty block would contradict
/// documented behaviour and leave a consumer unable to tell which canonical
/// list the findings were checked against. The expectation is read from
/// [`Vocabulary::canonical`] rather than a literal list, so the assertion
/// cannot drift from the shipped vocabulary.
#[rstest]
#[case::warn(PolicyMode::Warn)]
#[case::deny(PolicyMode::Deny)]
#[case::off(PolicyMode::Off)]
fn behaviour_report_advertises_the_canonical_vocabulary(#[case] mode: PolicyMode) {
    let context = ctx_for_command(command(
        &["purge"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    ));

    let report = check_behaviour(&context, mode);

    assert_eq!(report.vocabulary, Vocabulary::canonical());
}

/// The bypass remedy must name the attribute that actually moves the metadata.
///
/// Advising `#[arg(long = "...")]` would send authors down a path that leaves
/// `AgentInput.long` untouched, so the same finding would reappear while the
/// clap and generated-metadata surfaces diverged.
#[test]
fn bypass_flag_unknown_remedy_names_the_cli_long_attribute() {
    let mut cmd = command(
        &["purge"],
        InteractionMode::Interactive,
        MutationEffect::Delete,
    );
    cmd.bypass_flag = Some("--force".to_owned());
    cmd.inputs.push(input("recipient", "string"));
    let context = ctx_for_command(cmd);

    let report = check_behaviour(&context, PolicyMode::Warn);
    let message = report
        .results
        .iter()
        .find(|result| result.code == "bypass_flag_unknown")
        .expect("unknown bypass should produce a finding")
        .message
        .as_str();

    assert!(
        message.contains("cli_long"),
        "remedy should name `cli_long`, got {message}"
    );
    assert!(
        !message.contains("#[arg(long"),
        "remedy must not advise the `#[arg(long = ...)]` attribute, got {message}"
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
