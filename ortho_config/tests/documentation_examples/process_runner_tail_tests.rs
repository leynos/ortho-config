//! Regression for the diagnostic tail of a timed-out child's streams.
//!
//! The runner keeps the head of each stream for callers that parse it and a
//! separate bounded tail for diagnostics. A timeout must therefore report both
//! ends, must keep them distinguishable, and must say how many bytes the child
//! produced in total. Before the tail existed, a child whose useful output came
//! last reported only its opening bytes, which is how earlier deadline failures
//! lost the compile-admission queue lines that explained them.

use super::super::{Operation, run_with_policy};
use super::{cleanup_incomplete, deadline_exceeded, execution_policy, probe_command};
use anyhow::{Context, Result, ensure};
use std::time::Duration;

#[test]
fn timeout_diagnostic_keeps_bounded_output_and_truncation_evidence() -> Result<()> {
    let mut command = probe_command("probes::output_then_sleep_probe")?;
    let error = run_with_policy(
        &mut command,
        Operation("capture timeout probe"),
        execution_policy(Duration::from_millis(150), Duration::from_millis(700), 64),
    )
    .expect_err("the sleeping output probe should time out");
    let diagnostic = format!("{error:#}");

    ensure!(deadline_exceeded(&error), "timeout lost its typed cause");
    ensure!(
        !cleanup_incomplete(&error),
        "cleanup was unexpectedly incomplete"
    );
    ensure!(
        diagnostic.contains("capture timeout probe: subprocess timed out"),
        "timeout diagnostic missing: {diagnostic}"
    );
    ensure_timeout_stream(
        &diagnostic,
        "stdout",
        "STDOUT_TIMEOUT_SENTINEL",
        "STDOUT_TAIL_SENTINEL",
    )?;
    ensure_timeout_stream(
        &diagnostic,
        "stderr",
        "STDERR_TIMEOUT_SENTINEL",
        "STDERR_TAIL_SENTINEL",
    )?;
    ensure!(
        diagnostic.contains("correlated admission telemetry=unavailable"),
        "admission telemetry availability missing: {diagnostic}"
    );
    Ok(())
}

/// Assert one truncated stream's diagnostic keeps both ends and separates them.
///
/// The head proves the capture limit still applies; the tail proves the last
/// bytes survive truncation; the head-section check proves the two windows hold
/// different bytes rather than the same ones rendered twice.
fn ensure_timeout_stream(
    diagnostic: &str,
    stream: &str,
    head_sentinel: &str,
    tail_sentinel: &str,
) -> Result<()> {
    ensure!(
        diagnostic.contains(&format!("{stream}: bytes_seen=")),
        "{stream} byte-count evidence missing: {diagnostic}"
    );
    ensure!(
        diagnostic.contains("bytes_captured=64, truncated=true"),
        "{stream} truncation evidence missing: {diagnostic}"
    );
    let section = diagnostic
        .find(&format!("{stream}: bytes_seen="))
        .and_then(|start| diagnostic.get(start..))
        .with_context(|| format!("{stream} evidence section missing: {diagnostic}"))?;
    let tail_at = section
        .find("--- last 65536 bytes ---")
        .with_context(|| format!("{stream} diagnostic tail missing: {diagnostic}"))?;
    ensure!(
        section
            .get(tail_at..)
            .is_some_and(|tail| tail.contains(tail_sentinel)),
        "{stream} diagnostic tail omitted {tail_sentinel}: {diagnostic}"
    );
    let head_at = section
        .find("--- first 64 bytes ---")
        .with_context(|| format!("{stream} diagnostic head missing: {diagnostic}"))?;
    let head_section = section.get(head_at..tail_at).unwrap_or_default();
    ensure!(
        head_section.contains(head_sentinel),
        "{stream} diagnostic head omitted {head_sentinel}: {diagnostic}"
    );
    ensure!(
        !head_section.contains(tail_sentinel),
        "{stream} diagnostic head must not contain the tail marker: {diagnostic}"
    );
    Ok(())
}
