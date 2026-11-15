//! Bounded tracing evidence for the production bridge lifecycle.
//!
//! This helper is limited to bridge build, snapshot, execution, and cache
//! publication events. It does not expose paths, cache keys, or child output.

use std::time::Instant;

/// Record a completed bridge operation with a closed outcome vocabulary.
pub(crate) fn completed(operation: &'static str, outcome: &'static str, started: Instant) {
    tracing::debug!(
        target: "cargo_orthohelp::bridge",
        operation,
        outcome,
        elapsed_ms = started.elapsed().as_millis(),
        "bridge operation completed"
    );
}

/// Record a failed bridge operation without attaching unbounded error data.
pub(crate) fn failed(operation: &'static str, outcome: &'static str, started: Instant) {
    tracing::warn!(
        target: "cargo_orthohelp::bridge",
        operation,
        outcome,
        elapsed_ms = started.elapsed().as_millis(),
        "bridge operation failed"
    );
}
