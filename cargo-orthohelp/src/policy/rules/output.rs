//! Structured-output lint rules for the agent-native policy report.
//!
//! This module implements the `agent-native.output.json-missing` rule. It
//! inspects the *declared* structured-output contract of each command in a
//! compiled [`AgentContext`] and reports every command that says its normal
//! successful result is data (`stdout = "data"`) without claiming JSON support
//! for it.
//!
//! The rule reads only the `output` block. It never infers an output contract
//! from a command path, canonical verb, flags, renderer declaration, locale, or
//! the parent command that owns a nested command: an absent declaration is
//! undeclared, not a default. A declared `supports_json = false` is a claim
//! that JSON is unavailable, and therefore fails exactly like an undeclared
//! capability, while `supports_json = true` is the author's assertion that the
//! `--json` capability exists. Verifying that assertion against a real flag or
//! runtime is out of scope for this rule.
//!
//! Each finding's message is the entire operator experience: it names the
//! command path and gives the annotation to add *if the application really
//! implements JSON*, since agent context carries no source spans
//! (`location: None`).

use ortho_config::agent_context::StdoutPurpose;
use ortho_config::{AgentCommand, AgentContext};

use crate::policy::Vocabulary;
use crate::policy::{PolicyMode, PolicyReport, PolicyResult, PolicySeverity};

/// Stable policy rule identifier for a data command without JSON support.
const RULE_ID: &str = "agent-native.output.json-missing";

/// Machine-readable code emitted by [`RULE_ID`].
const CODE: &str = "json_support_missing";

/// Runs the structured-output rule over a compiled agent context.
///
/// The function is total: [`PolicyMode::Off`] returns an empty report without
/// evaluating the rule. Otherwise one finding is emitted for every command
/// declared to return data without JSON support, in command-path order.
/// Commands sharing a path keep their encounter order, and each occurrence is
/// reported: equal paths are never deduplicated.
///
/// Both the enforcing and the suppressed report advertise the canonical
/// vocabulary, matching [`check_behaviour`](super::behaviour::check_behaviour)
/// so a consumer parsing this rule set sees the same `vocabulary` block it
/// would see from any other policy report.
///
/// # Examples
///
/// A data command that never declares JSON support fails in `deny` mode. Run
/// both command checkers over the same context and read each report's deny
/// count, because a command can trip either rule set independently.
///
/// ```
/// use cargo_orthohelp::policy::PolicyMode;
/// use cargo_orthohelp::policy::rules::behaviour::check_behaviour;
/// use cargo_orthohelp::policy::rules::output::check_output;
/// use ortho_config::AgentContext;
///
/// let context: AgentContext = serde_json::from_value(serde_json::json!({
///     "schema_version": "1",
///     "kind": "demo.agent_context",
///     "package": "demo",
///     "commands": [{
///         "path": ["demo", "get"],
///         "interaction_mode": "non_interactive",
///         "mutation_effect": "read_only",
///         "output": { "stdout": "data" }
///     }]
/// }))
/// .expect("the example context should decode");
///
/// let behaviour = check_behaviour(&context, PolicyMode::Deny);
/// let output = check_output(&context, PolicyMode::Deny);
///
/// assert_eq!(behaviour.summary.deny, 0);
/// assert_eq!(output.summary.deny, 1);
/// assert_eq!(
///     output.results.first().map(|finding| finding.code.as_str()),
///     Some("json_support_missing")
/// );
/// ```
#[must_use]
pub fn check_output(context: &AgentContext, mode: PolicyMode) -> PolicyReport {
    if mode == PolicyMode::Off {
        return PolicyReport::with_details(mode, Vec::new(), Vec::new(), Vocabulary::canonical());
    }
    let mut results = Vec::new();
    collect_missing_json(context, mode, &mut results);
    PolicyReport::with_details(mode, results, Vec::new(), Vocabulary::canonical())
}

/// Walks the command tree in path order and records one finding per offender.
fn collect_missing_json(context: &AgentContext, mode: PolicyMode, out: &mut Vec<PolicyResult>) {
    let mut commands = context.commands.iter().collect::<Vec<_>>();
    commands.sort_by(|left, right| left.path.cmp(&right.path));
    for command in commands {
        if is_data_without_json(command) {
            finding(command, severity_for(mode), out);
        }
    }
}

/// Projects the enforcement mode onto the severity of one finding.
const fn severity_for(mode: PolicyMode) -> PolicySeverity {
    match mode {
        PolicyMode::Warn => PolicySeverity::Warn,
        _ => PolicySeverity::Deny,
    }
}

/// Whether `command` reports data without declaring JSON support.
///
/// Only an explicit `stdout = "data"` establishes data-returning status, and
/// only an explicit `supports_json = true` satisfies the rule: a refused and an
/// undeclared capability are both failures, while `path`, `summary`, `none`,
/// and the tolerant `unknown` fallback are not data at all.
fn is_data_without_json(command: &AgentCommand) -> bool {
    command.output.as_ref().is_some_and(|output| {
        output.stdout == Some(StdoutPurpose::Data) && output.supports_json != Some(true)
    })
}

/// Records the finding for one command.
fn finding(command: &AgentCommand, severity: PolicySeverity, out: &mut Vec<PolicyResult>) {
    out.push(PolicyResult {
        rule_id: RULE_ID.to_owned(),
        code: CODE.to_owned(),
        severity,
        message: missing_json_message(command),
        location: None,
    });
}

/// Explains the failing declaration and gives the annotation to add.
///
/// An explicit refusal and a forgotten declaration are told apart so the
/// author can see which mistake they made. Both messages lead with the JSON
/// remedy, because a data command that gains `--json` keeps its truthful
/// purpose; correcting `stdout` is offered only for a declaration that was
/// wrong about what the command returns.
fn missing_json_message(command: &AgentCommand) -> String {
    let state = match command
        .output
        .as_ref()
        .and_then(|output| output.supports_json)
    {
        Some(false) => "explicitly declines JSON support",
        _ => "leaves JSON support undeclared",
    };
    format!(
        "command `{}` declares stdout purpose `data` but {state}; add `output(supports_json = true)` to its arguments struct once it implements `--json`, or correct the declared stdout purpose if it does not return data",
        command.path.join(" ")
    )
}
