//! Run the private bridge executable and validate its JSON result.

use camino::Utf8Path;
use std::process::Command;
use std::time::Instant;

use crate::bridge_observability as observability;
use crate::error::OrthohelpError;

/// Execute the compiled bridge binary and return its stdout as JSON text.
#[tracing::instrument(skip_all, fields(operation = "bridge_execution"))]
pub(super) fn run_bridge(executable: &Utf8Path) -> Result<String, OrthohelpError> {
    let started = Instant::now();
    let output = Command::new(executable.as_str())
        .output()
        .map_err(|io_err| {
            observability::failed("bridge_execution", "spawn_failed", started);
            OrthohelpError::Io {
                path: executable.to_path_buf(),
                source: io_err,
            }
        })?;

    if !output.status.success() {
        observability::failed("bridge_execution", "nonzero_exit", started);
        let status = output.status.code().unwrap_or(-1);
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(OrthohelpError::BridgeExecutionFailure { status, message });
    }

    let json = String::from_utf8_lossy(&output.stdout).to_string();
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
