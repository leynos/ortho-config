//! Tie and occurrence coverage for the structured-output rule.
//!
//! Commands may share a path. Two properties then matter and are pinned here:
//! every occurrence is reported rather than deduplicated, and encounter order
//! survives while an explicit refusal and an undeclared capability remain
//! distinguishable through their message text. The truth table itself, which
//! varies JSON support, stdout purpose and enforcement mode, lives in
//! [`super::output_tests`].

use ortho_config::agent_context::StdoutPurpose;

use super::output::check_output;
use super::output_tests::{command_with_output, contract};
use crate::policy::PolicyMode;

/// Equal paths keep encounter order, and the tie is externally visible.
///
/// The two commands share one path but disagree about JSON support, so their
/// findings differ in exactly the words a tie could swap. A sort that lost
/// encounter order would therefore reverse the explicit refusal and the
/// undeclared capability rather than passing unnoticed.
#[test]
fn equal_paths_keep_encounter_order_and_stay_distinguishable() {
    let mut context = ortho_config::AgentContext::new("output-ties");
    context.commands.push(command_with_output(
        &["dup"],
        Some(contract(Some(false), Some(StdoutPurpose::Data))),
    ));
    context.commands.push(command_with_output(
        &["dup"],
        Some(contract(None, Some(StdoutPurpose::Data))),
    ));

    let report = check_output(&context, PolicyMode::Deny);
    let messages = report
        .results
        .iter()
        .map(|result| result.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages.len(),
        2,
        "both occurrences must be reported: {messages:?}"
    );
    assert!(
        messages
            .first()
            .is_some_and(|message| message.contains("declines"))
            && messages
                .get(1)
                .is_some_and(|message| message.contains("undeclared")),
        "the first-declared command must be reported first: {messages:?}"
    );
}

/// One finding is emitted per command occurrence, even for equal paths.
#[test]
fn equal_paths_are_reported_once_each() {
    let mut context = ortho_config::AgentContext::new("output-duplicates");
    for _ in 0..3 {
        context.commands.push(command_with_output(
            &["dup"],
            Some(contract(None, Some(StdoutPurpose::Data))),
        ));
    }

    let report = check_output(&context, PolicyMode::Deny);
    assert_eq!(
        report.results.len(),
        3,
        "each occurrence must be reported rather than deduplicated"
    );
    assert_eq!(
        report.summary.deny, 3,
        "the summary must count every occurrence"
    );
}
