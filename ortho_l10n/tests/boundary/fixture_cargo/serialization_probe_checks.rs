//! Post-conditions for the serialization probe regressions.
//!
//! Split from `serialization_probe_tests` to keep that file within the
//! repository's 400-line ceiling. Each check takes what a regression observed
//! and states what it must show; none of them runs a probe.

use super::{SECOND_SLEEP, read_marker};
use anyhow::{Result, ensure};
use cap_std::fs::Dir;
use std::process::Output;

/// Requires both probes to succeed, not to overlap, and the second to keep its
/// own full sleep.
pub(super) fn ensure_runs_were_serialized(
    directory: &Dir,
    first_output: &Output,
    second_output: &Output,
) -> Result<()> {
    ensure!(
        first_output.status.success() && second_output.status.success(),
        "both probes should exit successfully: first {:?}, second {:?}",
        first_output.status,
        second_output.status,
    );
    let first_exit = read_marker(directory, "first.exit")?;
    let second_start = read_marker(directory, "second.start")?;
    let second_exit = read_marker(directory, "second.exit")?;
    ensure!(
        second_start >= first_exit,
        "the second probe started {second_start} ms and the first exited {first_exit} ms; \
         nested runs overlapped instead of being serialized",
    );
    ensure!(
        second_exit.saturating_sub(second_start) >= SECOND_SLEEP.as_millis(),
        "the second probe ran for {} ms rather than its own {} ms sleep, so its \
         deadline covered time spent waiting for the lock",
        second_exit.saturating_sub(second_start),
        SECOND_SLEEP.as_millis(),
    );
    Ok(())
}

/// Requires the error to name the wait, and no child to have been spawned.
pub(super) fn ensure_wait_was_reported(error: &anyhow::Error, directory: &Dir) -> Result<()> {
    ensure!(
        error
            .to_string()
            .contains("waited for a sibling nested Cargo invocation"),
        "the error should name the wait that spent the allowance:\n{error:#}",
    );
    ensure!(
        !directory.exists("waiter.start"),
        "the runner spawned a child after the allowance had been spent waiting",
    );
    Ok(())
}

/// Requires the error to cite the shared allowance and the child to have been cut short.
pub(super) fn ensure_charged_wait_was_reported(
    error: &anyhow::Error,
    directory: &Dir,
) -> Result<()> {
    // The probe's own description contains the word "allowance", and a spawn
    // failure echoes it, so the check matches the deadline error's note instead.
    ensure!(
        error
            .to_string()
            .contains("bounded by the boundary contract's shared allowance"),
        "the error should report the shared allowance rather than a bare cap:\n{error:#}",
    );
    ensure!(
        directory.exists("charged.start"),
        "the child never started, so its deadline was not what ended the run",
    );
    ensure!(
        !directory.exists("charged.exit"),
        "the child finished its sleep, so its bound was not the remaining allowance",
    );
    Ok(())
}
