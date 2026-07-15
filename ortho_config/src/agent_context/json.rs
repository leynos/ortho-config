//! JSON rendering adapter for [`super::AgentContext`].
//!
//! This module keeps JSON formatting policy outside the agent-context schema
//! model while providing the compact and pretty renderings used by adapters.

use super::AgentContext;

/// Serializes an agent context as compact JSON with a trailing newline.
///
/// # Errors
///
/// Returns a [`serde_json::Error`] when serialization fails.
pub fn serialize_agent_context(context: &AgentContext) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string(context)?;
    json.push('\n');
    Ok(json)
}

/// Serializes an agent context as indented JSON without a trailing newline.
///
/// # Errors
///
/// Returns a [`serde_json::Error`] when serialization fails.
pub fn serialize_agent_context_pretty(context: &AgentContext) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(context)
}
