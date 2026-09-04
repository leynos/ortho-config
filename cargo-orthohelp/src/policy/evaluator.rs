//! Minimal agent-native policy evaluation over derived command vocabulary.
//!
//! The evaluator is deliberately pure: callers supply an [`AgentContext`] and
//! selected [`PolicyMode`], then adapt its findings to CLI output or tests.
//! This first rule establishes the report and enforcement boundary without
//! implementing the broader phase-7 policy set.

use ortho_config::AgentContext;

use super::{PolicyMode, PolicyResult, PolicySeverity};

const NON_CANONICAL_LEGACY_FLAG: &str = "is-legacy-mode";
const CANONICAL_FLAG_RULE_ID: &str = "agent-native.vocabulary.canonical-flag";
const NON_CANONICAL_FLAG_CODE: &str = "non_canonical_flag";

/// Evaluates the currently supported agent-native vocabulary rule.
///
/// `off` mode suppresses evaluation. `warn` and `deny` use the same rule but
/// assign their corresponding finding severity so the report summary and CLI
/// outcome remain mode-consistent.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::{PolicyMode, evaluate_agent_native};
/// use ortho_config::AgentContext;
///
/// let findings = evaluate_agent_native(&AgentContext::new("example"), &PolicyMode::Warn);
/// assert!(findings.is_empty());
/// ```
#[must_use]
pub fn evaluate_agent_native(context: &AgentContext, mode: &PolicyMode) -> Vec<PolicyResult> {
    if matches!(mode, PolicyMode::Off) {
        return Vec::new();
    }
    let severity = severity_for(mode);
    let mut results = Vec::new();
    for command in &context.commands {
        for input in &command.inputs {
            if input.long.as_deref() == Some(NON_CANONICAL_LEGACY_FLAG) {
                results.push(non_canonical_flag_result(severity.clone()));
            }
        }
    }
    results
}

const fn severity_for(mode: &PolicyMode) -> PolicySeverity {
    match mode {
        PolicyMode::Off => PolicySeverity::Off,
        PolicyMode::Warn => PolicySeverity::Warn,
        PolicyMode::Deny => PolicySeverity::Deny,
    }
}

fn non_canonical_flag_result(severity: PolicySeverity) -> PolicyResult {
    PolicyResult {
        rule_id: CANONICAL_FLAG_RULE_ID.to_owned(),
        code: NON_CANONICAL_FLAG_CODE.to_owned(),
        severity,
        message: "Use `--legacy-mode` instead of the non-canonical \
                  `--is-legacy-mode` flag."
            .to_owned(),
        location: None,
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the minimal policy evaluator.

    use super::*;
    use ortho_config::{AgentCommand, AgentContext, AgentInput, InteractionMode, MutationEffect};
    use rstest::rstest;

    #[rstest]
    #[case(PolicyMode::Off, None)]
    #[case(PolicyMode::Warn, Some(PolicySeverity::Warn))]
    #[case(PolicyMode::Deny, Some(PolicySeverity::Deny))]
    fn legacy_flag_uses_the_selected_mode_severity(
        #[case] mode: PolicyMode,
        #[case] expected_severity: Option<PolicySeverity>,
    ) {
        let results = evaluate_agent_native(&context_with_legacy_flag(), &mode);

        match expected_severity {
            Some(severity) => {
                assert_eq!(results.len(), 1);
                let result = results
                    .first()
                    .expect("a non-off policy mode should produce one finding");
                assert_eq!(result.rule_id, CANONICAL_FLAG_RULE_ID);
                assert_eq!(result.code, NON_CANONICAL_FLAG_CODE);
                assert_eq!(result.severity, severity);
                assert!(result.location.is_none());
            }
            None => assert!(results.is_empty()),
        }
    }

    #[test]
    fn canonical_vocabulary_has_no_findings() {
        let context = context_with_flag("legacy-mode");

        assert!(evaluate_agent_native(&context, &PolicyMode::Warn).is_empty());
    }

    fn context_with_legacy_flag() -> AgentContext {
        context_with_flag(NON_CANONICAL_LEGACY_FLAG)
    }

    fn context_with_flag(long: &str) -> AgentContext {
        let mut context = AgentContext::new("example");
        context.commands.push(AgentCommand {
            path: vec!["example".to_owned()],
            summary: None,
            canonical_verb: None,
            inputs: vec![AgentInput {
                name: long.to_owned(),
                long: Some(long.to_owned()),
                value_type: Some("bool".to_owned()),
                required: false,
                default: None,
                enum_values: Vec::new(),
            }],
            output_modes: Vec::new(),
            interaction_mode: InteractionMode::default(),
            mutation_effect: MutationEffect::default(),
            async_submission: None,
            delivery_route: None,
            pagination: None,
            examples: Vec::new(),
        });
        context
    }
}
