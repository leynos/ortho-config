//! Run the private bridge executable and validate its JSON result.

use camino::Utf8Path;
use std::process::{Command, Output};
use std::time::Instant;

use crate::bridge_observability as observability;
use crate::error::OrthohelpError;

#[cfg(test)]
#[path = "bridge_execution_tests.rs"]
mod tests;

/// Exit code reported when a process ended without an ordinary status code.
///
/// Unix reports `None` for a signal-terminated child, so a sentinel keeps that
/// case distinguishable from a real exit status while still failing the
/// success test.
const UNKNOWN_EXIT_STATUS: i32 = -1;

/// Captured output of one bridge invocation.
///
/// The exit code is an `i32` rather than an `ExitStatus` so the diagnosis below
/// can be exercised without constructing a platform-specific status value.
struct BridgeOutput {
    /// Ordinary process exit code, or [`UNKNOWN_EXIT_STATUS`].
    status: i32,
    /// Bytes the bridge wrote to standard output.
    stdout: Vec<u8>,
    /// Bytes the bridge wrote to standard error.
    stderr: Vec<u8>,
}

impl From<Output> for BridgeOutput {
    fn from(output: Output) -> Self {
        Self {
            status: output.status.code().unwrap_or(UNKNOWN_EXIT_STATUS),
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

/// Execute the compiled bridge binary and return its stdout as JSON text.
#[tracing::instrument(skip_all, fields(operation = "bridge_execution"))]
pub(super) fn run_bridge(executable: &Utf8Path) -> Result<String, OrthohelpError> {
    run_bridge_with(executable, |command| {
        command.output().map(BridgeOutput::from)
    })
}

/// Diagnose one bridge invocation whose process launch is supplied by `invoke`.
///
/// `invoke` is a parameter rather than a literal `Command::output` call so the
/// four outcomes below stay testable without contriving a child process for
/// each one: the production caller passes the real spawn, while tests supply
/// captured output. A no-op implementation would satisfy any assertion made
/// against the JSON helpers alone, which is why the seam sits here.
fn run_bridge_with(
    executable: &Utf8Path,
    invoke: impl FnOnce(&mut Command) -> std::io::Result<BridgeOutput>,
) -> Result<String, OrthohelpError> {
    let started = Instant::now();
    let mut command = Command::new(executable.as_str());
    let output = invoke(&mut command).map_err(|io_err| {
        observability::failed("bridge_execution", "spawn_failed", started);
        OrthohelpError::Io {
            path: executable.to_path_buf(),
            source: io_err,
        }
    })?;

    if output.status != 0 {
        observability::failed("bridge_execution", "nonzero_exit", started);
        return Err(failed_execution(&output));
    }
    render_bridge_json(&output.stdout, started)
}

/// Report a bridge that ran to completion but exited unsuccessfully.
///
/// Plain `json` keeps rustc's rendered diagnostics on the bridge's own streams
/// rather than in an inherited terminal, so both are retained here; a message
/// that dropped stderr would hide why the bridge failed.
fn failed_execution(output: &BridgeOutput) -> OrthohelpError {
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    OrthohelpError::BridgeExecutionFailure {
        status: output.status,
        message,
    }
}

/// Validate the bridge's JSON document and return it pretty-printed.
fn render_bridge_json(stdout: &[u8], started: Instant) -> Result<String, OrthohelpError> {
    let json = String::from_utf8_lossy(stdout).to_string();
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|error| {
        observability::failed("bridge_execution", "invalid_json", started);
        OrthohelpError::IrJson(error)
    })?;
    let pretty = serde_json::to_string_pretty(&value).map_err(|error| {
        observability::failed("bridge_execution", "json_render_failed", started);
        OrthohelpError::IrJson(error)
    })?;
    observability::completed("bridge_execution", "success", started);
    Ok(pretty)
}
