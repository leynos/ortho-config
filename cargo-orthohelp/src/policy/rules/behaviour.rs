//! Behaviour lint rules for the agent-native policy report.
//!
//! This module implements the four `agent-native.behaviour.*` rules that
//! inspect a compiled [`AgentContext`]: whether a destructive command declares
//! a confirmation bypass, whether an interactive command declares a bypass,
//! whether a declared bypass matches a real declared input, and whether
//! interaction/mutation metadata is declared at all. The entry point is the
//! total function [`check_behaviour`]; it never infers semantics from command
//! names or flags (design doc §8.1), so undeclared metadata stays undeclared.
//!
//! Each finding's message is the entire operator experience: it names the
//! command path and gives the exact `behaviour(...)` annotation to add, since
//! agent context carries no source spans (`location: None`).

use ortho_config::{AgentCommand, AgentContext, InteractionMode, MutationEffect};

use crate::policy::{PolicyMode, PolicyReport, PolicyResult, PolicySeverity};

/// Severity shared by every finding in a given mode.
///
/// In `warn` mode every finding is a warning; in `deny` mode every finding is
/// a hard failure, including omitted metadata (design doc §8.1: \"the same
/// omitted fields fail CI\").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleSeverity {
    Warn,
    Deny,
}

impl RuleSeverity {
    const fn as_policy(self) -> PolicySeverity {
        match self {
            Self::Warn => PolicySeverity::Warn,
            Self::Deny => PolicySeverity::Deny,
        }
    }
}

/// Runs the four behaviour rules over a compiled agent context.
///
/// The function is total: [`PolicyMode::Off`] returns an empty report without
/// evaluating any rule. Findings are emitted in command-path order.
#[must_use]
pub fn check_behaviour(context: &AgentContext, mode: PolicyMode) -> PolicyReport {
    if mode == PolicyMode::Off {
        return PolicyReport::empty(mode);
    }
    // `Off` is handled above, so only the two enforcing modes remain.
    let severity = match mode {
        PolicyMode::Warn => RuleSeverity::Warn,
        _ => RuleSeverity::Deny,
    };

    let mut results = Vec::new();
    for command in &context.commands {
        check_bypass_requirements(command, severity, &mut results);
        check_bypass_known(command, severity, &mut results);
        check_undeclared(command, severity, &mut results);
    }
    PolicyReport::with_results(mode, results)
}

/// One lint rule's stable identifiers.
struct Rule {
    rule_id: &'static str,
    code: &'static str,
}

impl Rule {
    const DESTRUCTIVE_BYPASS: Self = Self {
        rule_id: "agent-native.behaviour.destructive-bypass",
        code: "destructive_bypass_missing",
    };
    const PROMPT_BYPASS: Self = Self {
        rule_id: "agent-native.behaviour.prompt-bypass",
        code: "prompt_bypass_missing",
    };
    const BYPASS_UNKNOWN: Self = Self {
        rule_id: "agent-native.behaviour.bypass-unknown",
        code: "bypass_flag_unknown",
    };
    const UNDECLARED: Self = Self {
        rule_id: "agent-native.behaviour.undeclared",
        code: "interaction_unknown",
    };
    const UNDECLARED_MUTATION: Self = Self {
        rule_id: "agent-native.behaviour.undeclared",
        code: "mutation_unknown",
    };
}

/// Records a finding in the results vector.
fn finding(rule: &Rule, severity: RuleSeverity, message: String, out: &mut Vec<PolicyResult>) {
    out.push(PolicyResult {
        rule_id: rule.rule_id.to_owned(),
        code: rule.code.to_owned(),
        severity: severity.as_policy(),
        message,
        location: None,
    });
}

/// `destructive_bypass_missing`: a destructive command must declare a bypass.
///
/// A command declared `non_interactive` is exempt: it cannot prompt, so the
/// declaration itself is the approved non-interactive path (decision log).
fn check_bypass_requirements(
    command: &AgentCommand,
    severity: RuleSeverity,
    out: &mut Vec<PolicyResult>,
) {
    if command.bypass_flag.is_some() {
        return;
    }
    let path = command.path.join(" ");
    if command.mutation_effect == MutationEffect::Delete
        && command.interaction_mode != InteractionMode::NonInteractive
    {
        finding(
            &Rule::DESTRUCTIVE_BYPASS,
            severity,
            format!(
                "command `{path}` is destructive but declares no bypass flag; add `behaviour(bypass = \"--force\")` to its arguments struct"
            ),
            out,
        );
    }
    if command.interaction_mode == InteractionMode::Interactive {
        finding(
            &Rule::PROMPT_BYPASS,
            severity,
            format!(
                "command `{path}` may prompt but declares no bypass flag; add `behaviour(bypass = \"--force\")` to its arguments struct"
            ),
            out,
        );
    }
}

/// `bypass_flag_unknown`: a declared bypass must match a declared input's long
/// flag. This is contradiction detection between two declarations, not name
/// inference.
fn check_bypass_known(command: &AgentCommand, severity: RuleSeverity, out: &mut Vec<PolicyResult>) {
    let Some(bypass) = command.bypass_flag.as_deref() else {
        return;
    };
    let declared_flags = command
        .inputs
        .iter()
        .filter_map(|input| input.long.as_deref())
        .collect::<Vec<_>>();
    if declared_flags.contains(&bypass.strip_prefix("--").unwrap_or(bypass)) {
        return;
    }
    finding(
        &Rule::BYPASS_UNKNOWN,
        severity,
        format!(
            "command `{}` declares bypass flag `{bypass}` but no input exposes it; add a matching `#[arg(long = \"{}\")]` input or change the declared bypass",
            command.path.join(" "),
            bypass.strip_prefix("--").unwrap_or(bypass)
        ),
        out,
    );
}

/// `interaction_unknown` / `mutation_unknown`: omitted metadata is undeclared.
fn check_undeclared(command: &AgentCommand, severity: RuleSeverity, out: &mut Vec<PolicyResult>) {
    if command.interaction_mode == InteractionMode::Unknown {
        finding(
            &Rule::UNDECLARED,
            severity,
            format!(
                "command `{}` has undeclared interaction behaviour; add `behaviour(interaction = \"non_interactive\")` or `behaviour(interaction = \"interactive\")` to its arguments struct",
                command.path.join(" ")
            ),
            out,
        );
    }
    if command.mutation_effect == MutationEffect::Unknown {
        finding(
            &Rule::UNDECLARED_MUTATION,
            severity,
            format!(
                "command `{}` has undeclared mutation boundary; add `behaviour(mutation = \"read_only\")` or `behaviour(mutation = \"delete\")` to its arguments struct",
                command.path.join(" ")
            ),
            out,
        );
    }
}
