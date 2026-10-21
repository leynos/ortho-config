//! The compact agent-context JSON document writers.
//!
//! Split from the parent `output` module because the workspace caps a module
//! at 400 lines, and the agent-context path carries a serialization step, an
//! atomic write and three diagnostic emissions of its own. Every writer here
//! goes through the parent's atomic-write machinery; nothing about the file
//! layout or the fsync ordering changes by living in this file.

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::OrthohelpError;
use ortho_config::AgentContext;

use super::{write_atomic_json, write_json_stdout};

/// Writes the compact agent-context JSON document.
///
/// # Errors
/// Returns a JSON error on serialization or an I/O error on the atomic write.
pub fn write_agent_context(
    out_dir: &Utf8Path,
    payload: &AgentContext,
) -> Result<Utf8PathBuf, OrthohelpError> {
    log_agent_context_start(&payload.package, out_dir);
    let content = serialize_agent_context(out_dir, payload)?;
    let path = write_atomic_json(out_dir, "agent-context.json", &content, "agent-context")?;
    log_agent_context_written(&payload.package, &path, content.len());
    Ok(path)
}

/// Writes one compact agent-context JSON document to standard output.
///
/// The document is followed by a newline so it can be consumed as one complete
/// machine-readable result.
///
/// # Examples
///
/// This writes an empty `example-cli` context with no commands as one JSON
/// line.
///
/// ```no_run
/// use cargo_orthohelp::output::write_agent_context_stdout;
/// use ortho_config::AgentContext;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     write_agent_context_stdout(&AgentContext::new("example-cli"))?;
///     Ok(())
/// }
/// ```
///
/// # Errors
/// Returns a JSON error on serialization or an I/O error when writing stdout.
pub fn write_agent_context_stdout(payload: &AgentContext) -> Result<(), OrthohelpError> {
    write_json_stdout("agent-context", payload)
}

/// Announce the start of an agent-context write.
///
/// Each emission lives in its own function so that
/// [`write_agent_context`] holds at most one `tracing` call: when a dependent
/// crate enables `tracing`'s `log` bridge, Cargo's feature unification
/// expands every event into a branch ladder that counts against the
/// workspace's cognitive-complexity ceiling.
fn log_agent_context_start(package: &str, out_dir: &Utf8Path) {
    tracing::debug!(
        package = %package,
        out_dir = %out_dir,
        "starting agent-context write",
    );
}

/// Announce a completed agent-context write with its size.
fn log_agent_context_written(package: &str, path: &Utf8Path, bytes: usize) {
    tracing::debug!(
        package = %package,
        path = %path,
        bytes,
        "agent-context JSON written successfully",
    );
}

/// Serialize the agent context, reporting a failure with its input directory.
fn serialize_agent_context(
    out_dir: &Utf8Path,
    payload: &AgentContext,
) -> Result<String, OrthohelpError> {
    serde_json::to_string_pretty(payload).map_err(|error| {
        log_agent_context_serialization_failure(&payload.package, out_dir, &error);
        OrthohelpError::IrJson(error)
    })
}

/// Report a serialization failure before it is returned to the caller.
///
/// The event carries the destination directory under the field name `path`
/// to match the emission this module inherited from the former
/// `output.rs`, so existing `path` filters still match.
fn log_agent_context_serialization_failure(
    package: &str,
    out_dir: &Utf8Path,
    error: &serde_json::Error,
) {
    tracing::debug!(
        package = %package,
        path = %out_dir,
        error = %error,
        "failed to serialize agent-context JSON",
    );
}
