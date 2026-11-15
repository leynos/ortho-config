//! Complete truth table and negative controls for the structured-output rule.
//!
//! The expectations here are a restatement of the policy specification, not a
//! call to the production predicate: every combination of JSON support,
//! stdout purpose, and enforcement mode is named, together with the verdict
//! the specification requires. A rule that stops firing, fires for the wrong
//! state, or ignores the enforcement mode therefore fails the table rather
//! than agreeing with itself.

use insta::assert_snapshot;
use ortho_config::agent_context::{ExitClass, OutputContract, StdoutPurpose};
use ortho_config::{AgentCommand, InteractionMode, MutationEffect};
use rstest::rstest;

use super::output::check_output;
use super::tests::{command, ctx_for_command};
use crate::policy::{PolicyMode, PolicyReport, PolicyResult, PolicySeverity, Vocabulary};

/// Builds a contract with exactly the supplied capabilities.
///
/// The unrelated declarations stay empty so each test varies one axis at a
/// time; tests that deliberately vary them build their own contracts.
pub(super) const fn contract(
    supports_json: Option<bool>,
    stdout: Option<StdoutPurpose>,
) -> OutputContract {
    OutputContract {
        supports_json,
        stdout,
        stderr_diagnostics: None,
        exit_classes: Vec::new(),
    }
}

/// Attaches an optional output block to an otherwise inert command.
///
/// A non-interactive, read-only declaration keeps the behaviour rules quiet,
/// so any finding in a report comes from the structured-output rule alone.
pub(super) fn command_with_output(path: &[&str], output: Option<OutputContract>) -> AgentCommand {
    let mut cmd = command(
        path,
        InteractionMode::NonInteractive,
        MutationEffect::ReadOnly,
    );
    cmd.output = output;
    cmd
}

/// Checks one command in one enforcement mode.
fn report_for(output: Option<OutputContract>, mode: PolicyMode) -> PolicyReport {
    check_output(
        &ctx_for_command(command_with_output(&["probe"], output)),
        mode,
    )
}

/// The severity every finding carries in an enforcing mode.
fn expected_severity(mode: PolicyMode) -> PolicySeverity {
    match mode {
        PolicyMode::Warn => PolicySeverity::Warn,
        _ => PolicySeverity::Deny,
    }
}

/// Asserts that a report holds exactly the one expected finding.
fn assert_single_finding(report: &PolicyReport, mode: PolicyMode) {
    assert_eq!(
        report.results.len(),
        1,
        "expected exactly one finding in {mode} mode, got {:#?}",
        report.results
    );
    let finding = report
        .results
        .first()
        .expect("a report with one result has a first result");
    assert_stable_fields(finding, mode);
    assert_message_fields(finding);
    assert_summary_counts(report, mode);
}

/// Asserts the machine-readable fields consumers are entitled to rely on.
fn assert_stable_fields(finding: &PolicyResult, mode: PolicyMode) {
    assert_eq!(
        finding.rule_id, "agent-native.output.json-missing",
        "the stable rule identifier is part of the contract"
    );
    assert_eq!(
        finding.code, "json_support_missing",
        "the machine-readable code is part of the contract"
    );
    assert_eq!(
        finding.severity,
        expected_severity(mode),
        "the finding severity must follow the enforcement mode"
    );
    assert!(
        finding.location.is_none(),
        "agent context carries no source spans, so location stays absent"
    );
}

/// Asserts that the explanation identifies the command, the capability, and
/// the remedy.
fn assert_message_fields(finding: &PolicyResult) {
    let expected_fragments = ["`probe`", "--json", "output(supports_json = true)"];
    for fragment in expected_fragments {
        assert!(
            finding.message.contains(fragment),
            "the explanation must contain {fragment}: {}",
            finding.message
        );
    }
}

/// Asserts that the summary counts the single finding under its severity.
fn assert_summary_counts(report: &PolicyReport, mode: PolicyMode) {
    assert_eq!(
        report.summary.total, 1,
        "the summary must count the one finding"
    );
    match expected_severity(mode) {
        PolicySeverity::Warn => assert_eq!(
            report.summary.warn, 1,
            "the warning count must match the finding"
        ),
        _ => assert_eq!(
            report.summary.deny, 1,
            "the deny count must match the finding"
        ),
    }
}

/// Asserts that a report holds no findings and an empty summary.
fn assert_no_findings(report: &PolicyReport) {
    assert!(
        report.results.is_empty(),
        "expected no findings, got {:#?}",
        report.results
    );
    assert_eq!(report.summary.total, 0, "the summary total must be zero");
    assert_eq!(report.summary.warn, 0, "the warning count must be zero");
    assert_eq!(report.summary.deny, 0, "the deny count must be zero");
}

/// Every (JSON support, stdout purpose, mode) combination, exactly once.
///
/// The verdict is derived from the specification: one finding per command
/// occurrence if and only if stdout is declared `data`, JSON support is not
/// `true`, and enforcement is not off.
#[rstest]
fn complete_rule_table(
    #[values(None, Some(false), Some(true))] supports_json: Option<bool>,
    #[values(
        None,
        Some(StdoutPurpose::Data),
        Some(StdoutPurpose::Path),
        Some(StdoutPurpose::Summary),
        Some(StdoutPurpose::None),
        Some(StdoutPurpose::Unknown)
    )]
    stdout: Option<StdoutPurpose>,
    #[values(PolicyMode::Off, PolicyMode::Warn, PolicyMode::Deny)] mode: PolicyMode,
) {
    let report = report_for(Some(contract(supports_json, stdout)), mode);
    let should_fire = stdout == Some(StdoutPurpose::Data)
        && supports_json != Some(true)
        && mode != PolicyMode::Off;

    assert_eq!(
        report.mode, mode,
        "the report must echo the requested enforcement mode"
    );
    assert_eq!(
        report.vocabulary,
        Vocabulary::canonical(),
        "every report advertises the canonical vocabulary"
    );
    assert_eq!(
        report.summary.total,
        report.results.len(),
        "the summary total must equal the result count"
    );
    if should_fire {
        assert_single_finding(&report, mode);
    } else {
        assert_no_findings(&report);
    }
}

/// A present but empty output block makes no claim and cannot fire.
///
/// This separates "declared nothing" from "declared a record whose leaves are
/// absent", which the wire form preserves as `{}`.
#[test]
fn present_empty_block_produces_no_finding() {
    let report = report_for(Some(contract(None, None)), PolicyMode::Deny);
    assert_no_findings(&report);
}

/// An absent output block never becomes a defaulted record.
#[test]
fn absent_block_produces_no_finding() {
    let report = report_for(None, PolicyMode::Deny);
    assert_no_findings(&report);
}

/// A canonical verb is not evidence about stdout or JSON support.
///
/// `get` is exactly the verb that invites inference, so the command declares
/// it while declaring a non-data purpose. The rule must stay silent.
#[test]
fn canonical_verb_is_never_inferred_as_data() {
    let mut cmd = command_with_output(&["get"], Some(contract(None, Some(StdoutPurpose::Summary))));
    cmd.canonical_verb = Some("get".to_owned());
    let report = check_output(&ctx_for_command(cmd), PolicyMode::Deny);
    assert_no_findings(&report);
}

/// A command named `get` with no output declaration is still undeclared.
#[test]
fn command_named_get_without_declaration_produces_no_finding() {
    let mut cmd = command_with_output(&["get"], None);
    cmd.canonical_verb = Some("get".to_owned());
    let report = check_output(&ctx_for_command(cmd), PolicyMode::Deny);
    assert_no_findings(&report);
}

/// False and undeclared JSON support fail but must be explained differently.
///
/// The messages differ in the state they report and agree on the remedy, so an
/// author can tell a deliberate refusal from a forgotten declaration.
#[test]
fn false_and_undeclared_json_support_have_distinct_explanations() {
    let false_report = report_for(
        Some(contract(Some(false), Some(StdoutPurpose::Data))),
        PolicyMode::Deny,
    );
    let undeclared_report = report_for(
        Some(contract(None, Some(StdoutPurpose::Data))),
        PolicyMode::Deny,
    );
    let false_message = false_report
        .results
        .first()
        .expect("explicit refusal should produce a finding")
        .message
        .as_str();
    let undeclared_message = undeclared_report
        .results
        .first()
        .expect("an undeclared capability should produce a finding")
        .message
        .as_str();

    assert!(
        false_message.contains("declines"),
        "an explicit refusal must be reported as a refusal: {false_message}"
    );
    assert!(
        undeclared_message.contains("undeclared"),
        "a missing declaration must be reported as undeclared: {undeclared_message}"
    );
    assert_ne!(
        false_message, undeclared_message,
        "the two failing declarations must not share one explanation"
    );
}

/// `stderr_diagnostics` and `exit_classes` cannot change the rule's verdict.
///
/// The baseline and the varied contract differ only in those unrelated leaves,
/// so any difference in the reports would be an inference the rule must not
/// make.
#[rstest]
fn unrelated_output_fields_do_not_change_the_rule(
    #[values(None, Some(false), Some(true))] stderr_diagnostics: Option<bool>,
    #[values(0usize, 1, 2)] class_count: usize,
    #[values(false, true)] is_data: bool,
) {
    let stdout = if is_data {
        Some(StdoutPurpose::Data)
    } else {
        Some(StdoutPurpose::Summary)
    };
    let baseline = contract(Some(false), stdout);
    let varied = OutputContract {
        stderr_diagnostics,
        exit_classes: (0..class_count)
            .map(|index| ExitClass {
                name: format!("class{index}"),
                meaning: "generated".to_owned(),
            })
            .collect(),
        ..baseline.clone()
    };

    let expected = report_for(Some(baseline), PolicyMode::Deny);
    let actual = report_for(Some(varied), PolicyMode::Deny);

    assert_eq!(
        actual.results, expected.results,
        "unrelated declarations must not affect the rule"
    );
    assert_eq!(
        actual.results.len(),
        usize::from(is_data),
        "only a declared data purpose may fire"
    );
}

/// Findings follow command-path order, not insertion order.
#[test]
fn findings_follow_command_path_order() {
    let mut context = ortho_config::AgentContext::new("output-order");
    context.commands.push(command_with_output(
        &["zebra"],
        Some(contract(None, Some(StdoutPurpose::Data))),
    ));
    context.commands.push(command_with_output(
        &["alpha"],
        Some(contract(None, Some(StdoutPurpose::Data))),
    ));

    let report = check_output(&context, PolicyMode::Warn);
    let paths = report
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

    assert_eq!(
        paths,
        ["alpha", "zebra"],
        "findings must be ordered by command path"
    );
}

/// A context that exercises a firing command beside two silent declarations.
///
/// `export` trips the rule, `inspect` claims data with declared JSON support,
/// and `locate` declares a non-data purpose. The snapshots therefore show one
/// finding and no findings together, rather than a report that would look the
/// same whether or not the silent cases were evaluated.
fn report_snapshot_context() -> ortho_config::AgentContext {
    let mut context = ortho_config::AgentContext::new("output-snapshot");
    context.commands.push(command_with_output(
        &["export"],
        Some(contract(None, Some(StdoutPurpose::Data))),
    ));
    context.commands.push(command_with_output(
        &["inspect"],
        Some(contract(Some(true), Some(StdoutPurpose::Data))),
    ));
    context.commands.push(command_with_output(
        &["locate"],
        Some(contract(None, Some(StdoutPurpose::Path))),
    ));
    context
}

/// Each enforcement mode's whole report is locked as a snapshot.
///
/// `off` must serialize an empty result set while still echoing the mode and
/// the canonical vocabulary; `warn` and `deny` differ in severity and summary
/// counts for the same finding. Locking the document catches a reworded
/// remedy, a changed identifier, or a summary that drifts from the results.
#[rstest]
#[case::off(PolicyMode::Off)]
#[case::warn(PolicyMode::Warn)]
#[case::deny(PolicyMode::Deny)]
fn output_report_snapshots_lock_each_mode(#[case] mode: PolicyMode) {
    let report = check_output(&report_snapshot_context(), mode);
    let json = serde_json::to_string_pretty(&report).expect("serialize the output report");

    assert_snapshot!(format!("output_report__{mode}"), json);
}
