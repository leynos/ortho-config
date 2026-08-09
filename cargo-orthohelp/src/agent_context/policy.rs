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
