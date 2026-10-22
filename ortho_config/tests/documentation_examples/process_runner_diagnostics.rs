//! Bounded, secret-free subprocess failure evidence.

use super::RunContext;
use super::capture::{Reader, Snapshot};
use std::ffi::OsStr;
use std::fmt::{self, Write as _};
use std::path::PathBuf;
use std::process::Command;

#[cfg(test)]
#[path = "process_runner_diagnostics_tests.rs"]
mod tests;

pub(super) fn capture_snapshots(readers: &[Reader]) -> Vec<(&'static str, Snapshot)> {
    readers
        .iter()
        .map(|reader| (reader.stream(), reader.snapshot()))
        .collect()
}

pub(super) fn stream_bytes(captures: &[(&'static str, Snapshot)], stream: &str) -> Vec<u8> {
    captures
        .iter()
        .find_map(|(name, capture)| (*name == stream).then(|| capture.bytes.clone()))
        .unwrap_or_default()
}

pub(super) fn format_failure_diagnostic(
    context: &RunContext,
    captures: &[(&'static str, Snapshot)],
    cleanup_evidence: &[String],
) -> Result<String, fmt::Error> {
    let elapsed = context.started.elapsed();
    let mut diagnostic = String::new();
    write!(
        &mut diagnostic,
        concat!(
            "documentation subprocess evidence: operation={:?}, kind={}, ",
            "workspace={}, process_id={}, elapsed_ms={}, ",
            "effective_deadline_ms={}, cleanup_deadline_ms={}, ",
            "output_limit_bytes_per_stream={}, {}; ",
            "correlated admission telemetry=unavailable; ",
            "structured compiler progress=unavailable at this process boundary"
        ),
        context.operation,
        context.policy.kind_name(),
        context.workspace,
        context.process_id,
        elapsed.as_millis(),
        context.policy.timeout().as_millis(),
        context.policy.cleanup_timeout().as_millis(),
        context.policy.output_limit(),
        context.environment
    )?;
    for stream in ["stdout", "stderr"] {
        append_stream_evidence(&mut diagnostic, stream, captures)?;
    }
    append_cleanup_evidence(&mut diagnostic, cleanup_evidence);
    Ok(diagnostic)
}

fn append_stream_evidence(
    diagnostic: &mut String,
    stream: &str,
    captures: &[(&'static str, Snapshot)],
) -> Result<(), fmt::Error> {
    let selected_capture = captures
        .iter()
        .find_map(|(name, capture)| (*name == stream).then_some(capture));
    match selected_capture {
        Some(snapshot) => write!(
            diagnostic,
            "\n{stream}: bytes_seen={}, bytes_captured={}, truncated={}\n{}",
            snapshot.bytes_seen,
            snapshot.bytes.len(),
            snapshot.truncated,
            String::from_utf8_lossy(&snapshot.bytes),
        ),
        None => write!(diagnostic, "\n{stream}: capture unavailable"),
    }
}

fn append_cleanup_evidence(diagnostic: &mut String, evidence: &[String]) {
    if evidence.is_empty() {
        return;
    }
    diagnostic.push_str("\ncleanup evidence:");
    for item in evidence {
        diagnostic.push_str("\n- ");
        diagnostic.push_str(item);
    }
}

/// Name the directory a subprocess runs in, never failing to report something.
///
/// A command-local directory wins because it overrides inheritance. When it is
/// absent the caller's `query` supplies the inherited working directory, and a
/// failed query is reported as such: the previous `ok()` chain collapsed "the
/// query failed" into the same string as "no directory is configured", which
/// hides an environmental fault exactly when the failure diagnostic matters.
///
/// The query is a parameter rather than an `std::env::current_dir` call so the
/// fallible environmental read stays injectable. The production caller passes
/// the real process query; tests supply their own without provoking a
/// host-wide environmental change.
pub(super) fn workspace_identity(
    command: &Command,
    query: impl FnOnce() -> std::io::Result<PathBuf>,
) -> String {
    // `map_or_else` takes the fallback lazily, so a command-local directory is
    // still reported without ever consulting the injected directory query.
    command.get_current_dir().map_or_else(
        || inherited_workspace_identity(query),
        |path| path.display().to_string(),
    )
}

/// Render the inherited working directory, naming an explicit query failure.
fn inherited_workspace_identity(query: impl FnOnce() -> std::io::Result<PathBuf>) -> String {
    query().map_or_else(
        |error| {
            format!("unavailable: querying the inherited working directory failed with {error}")
        },
        |path| path.display().to_string(),
    )
}

pub(super) fn environment_evidence(command: &Command) -> String {
    let keys = [
        "RUSTC_WRAPPER",
        "RUSTDOC",
        "CARGO_BUILD_JOBS",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
    ];
    let evidence = keys
        .iter()
        .map(|key| format!("{key}={}", environment_presence(command, key)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "safe process-environment evidence=[{evidence}]; effective Cargo configuration values=unavailable at process boundary"
    )
}

fn environment_presence(command: &Command, name: &str) -> &'static str {
    if let Some((_, value)) = command.get_envs().find(|(key, _)| *key == OsStr::new(name)) {
        return if value.is_some() {
            "command-set"
        } else {
            "command-removed"
        };
    }
    if std::env::var_os(name).is_some() {
        "parent-present; child inheritance unknown"
    } else {
        "parent-absent; Cargo config may still apply"
    }
}
