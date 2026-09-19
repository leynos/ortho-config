//! Policy mapping from the tool's configuration model to agent-context wire.
//!
//! Owns the single-point conversions (Decision D12) so the two mirror types
//! cannot drift silently, and the D9 application rule that decides which mode
//! the generated agent context advertises.

use ortho_config::agent_context::{
    AgentContext, AgentPolicy, PolicyException as PolicyExceptionWire, PolicyMode as PolicyModeWire,
};

use crate::policy::PolicyMode as PolicyModeTool;
use crate::policy::config::{PolicyConfig, PolicyException as PolicyExceptionTool};

impl From<PolicyModeTool> for PolicyModeWire {
    fn from(mode: PolicyModeTool) -> Self {
        match mode {
            PolicyModeTool::Off => Self::Off,
            PolicyModeTool::Warn => Self::Warn,
            PolicyModeTool::Deny => Self::Deny,
        }
    }
}

impl From<&PolicyExceptionTool> for PolicyExceptionWire {
    fn from(exception: &PolicyExceptionTool) -> Self {
        Self {
            kind: exception.kind.to_string(),
            name: exception.name.clone(),
            command_path: exception.command_path.clone(),
        }
    }
}

/// Applies the configured policy to a generated agent context.
///
/// Decision D9: keep the advertised default (`warn`) when no policy table
/// exists, and use the configured mode with its exceptions when one does. The
/// transient `--policy-mode` override never reaches the context, which records
/// what the project has committed to. Reasons are not copied into the
/// agent-distributed artefact (Decision D12).
pub fn apply_policy_to_context(context: &mut AgentContext, policy: Option<&PolicyConfig>) {
    context.policy = policy.map_or_else(AgentPolicy::default, |config| AgentPolicy {
        agent_native: config.mode.into(),
        exceptions: config.exceptions.iter().map(Into::into).collect(),
    });
}

#[cfg(test)]
mod tests {
    //! Tests for policy conversion into the agent-context wire contract.

    use super::*;
    use crate::policy::config::ExceptionKind;
    use rstest::rstest;

    #[rstest]
    fn absent_policy_uses_the_agent_context_default() {
        let mut context = AgentContext::new("demo");

        apply_policy_to_context(&mut context, None);

        assert_eq!(context.policy, AgentPolicy::default());
    }

    #[rstest]
    #[case(PolicyModeTool::Off, PolicyModeWire::Off)]
    #[case(PolicyModeTool::Warn, PolicyModeWire::Warn)]
    #[case(PolicyModeTool::Deny, PolicyModeWire::Deny)]
    fn configured_policy_preserves_the_selected_mode(
        #[case] mode: PolicyModeTool,
        #[case] expected: PolicyModeWire,
    ) {
        let config = PolicyConfig {
            mode,
            exceptions: Vec::new(),
        };
        let mut context = AgentContext::new("demo");

        apply_policy_to_context(&mut context, Some(&config));

        assert_eq!(context.policy.agent_native, expected);
    }

    #[rstest]
    fn configured_exception_excludes_its_reason_from_the_wire_contract() {
        let config = PolicyConfig {
            mode: PolicyModeTool::Warn,
            exceptions: vec![PolicyExceptionTool {
                kind: ExceptionKind::Flag,
                name: "--legacy".to_owned(),
                reason: "compatibility".to_owned(),
                command_path: Some("demo publish".to_owned()),
            }],
        };
        let mut context = AgentContext::new("demo");

        apply_policy_to_context(&mut context, Some(&config));

        let exception = context
            .policy
            .exceptions
            .first()
            .expect("configured policy should produce one wire exception");
        assert_eq!(exception.kind, "flag");
        assert_eq!(exception.name, "--legacy");
        assert_eq!(exception.command_path.as_deref(), Some("demo publish"));
        let wire = serde_json::to_value(exception).expect("serialize policy exception");
        assert!(wire.get("reason").is_none());
    }
}
