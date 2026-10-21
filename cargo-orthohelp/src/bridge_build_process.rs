//! Supervise the Cargo child and classify its structured build result.

use camino::Utf8Path;
use std::io::BufReader;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

use super::cleanup::OwnedChild;
use super::diagnostics::forward_cargo_diagnostics;
use super::protocol::{CargoMessages, read_cargo_messages};
use super::{
    CargoBuild, CargoFailure, CargoStatus, GeneratedTarget, io_error, message_error,
    with_cleanup_evidence,
};
use crate::bridge_observability as observability;
use crate::error::OrthohelpError;

/// Run Cargo with a piped JSON stream and return its status and parsed messages.
#[tracing::instrument(skip_all, fields(operation = "bridge_cargo_build"))]
pub(super) fn run_cargo(
    command: &mut Command,
    target: GeneratedTarget<'_>,
) -> Result<CargoBuild, CargoFailure> {
    let started = Instant::now();
    let mut child = spawn_cargo(command, target.directory, started)?;
    let messages = read_cargo_protocol(&mut child, target, started)?;
    let status = wait_for_cargo(&mut child, target.directory, started)?;
    forward_diagnostics(&messages);
    record_outcome(&messages, status, started);
    Ok(CargoBuild {
        status: cargo_status(status),
        messages,
    })
}

/// Spawn Cargo with the output streams expected by the protocol reader.
fn spawn_cargo(
    command: &mut Command,
    target_directory: &Utf8Path,
    started: Instant,
) -> Result<OwnedChild, CargoFailure> {
    command.stdout(Stdio::piped()).stderr(Stdio::inherit());
    command.spawn().map(OwnedChild::new).map_err(|error| {
        observability::failed("bridge_cargo_build", "spawn_failed", started);
        CargoFailure::confirmed(io_error(target_directory, error))
    })
}

/// Read Cargo's JSON protocol, cleaning up the owned child on read failure.
fn read_cargo_protocol(
    child: &mut OwnedChild,
    target: GeneratedTarget<'_>,
    started: Instant,
) -> Result<CargoMessages, CargoFailure> {
    let Some(stdout) = child.child.stdout.take() else {
        return Err(cleanup_failure(
            child,
            message_error("Cargo build stdout was not piped"),
            "stdout_unavailable",
            started,
        ));
    };
    let mut reader = BufReader::new(stdout);
    let result = read_cargo_messages(&mut reader, &target);
    drop(reader);
    result.map_err(|error| {
        cleanup_failure(
            child,
            io_error(target.directory, error),
            "protocol_read_failed",
            started,
        )
    })
}

/// Wait for Cargo and clean up the owned process if waiting fails.
fn wait_for_cargo(
    child: &mut OwnedChild,
    target_directory: &Utf8Path,
    started: Instant,
) -> Result<ExitStatus, CargoFailure> {
    child.wait().map_err(|error| {
        cleanup_failure(
            child,
            io_error(target_directory, error),
            "wait_failed",
            started,
        )
    })
}

/// Preserve a primary error while retaining bounded child-cleanup evidence.
fn cleanup_failure(
    child: &mut OwnedChild,
    primary: OrthohelpError,
    outcome: &'static str,
    started: Instant,
) -> CargoFailure {
    observability::failed("bridge_cargo_build", outcome, started);
    let cleanup = child.stop_bounded();
    CargoFailure::unconfirmed(with_cleanup_evidence(primary, &cleanup))
}

/// Forward retained compiler diagnostics without exposing them as log fields.
fn forward_diagnostics(messages: &CargoMessages) {
    if let Err(error) = forward_cargo_diagnostics(messages) {
        tracing::warn!(
            operation = "cargo_diagnostic_forward",
            outcome = "failed",
            error_kind = ?error.kind(),
            "could not forward bounded Cargo diagnostics to stderr"
        );
    }
}

/// Record a bounded event for Cargo's process and protocol outcome.
fn record_outcome(messages: &CargoMessages, status: ExitStatus, started: Instant) {
    let outcome = if messages.protocol_error.is_some() {
        "protocol_error"
    } else if status.success() {
        "success"
    } else {
        "nonzero_exit"
    };
    if outcome == "success" {
        observability::completed("bridge_cargo_build", outcome, started);
    } else {
        observability::failed("bridge_cargo_build", outcome, started);
    }
}

/// Convert the operating-system status into the bridge build result.
fn cargo_status(status: ExitStatus) -> CargoStatus {
    if status.success() {
        CargoStatus::Success
    } else {
        CargoStatus::Failure(status.code().unwrap_or(-1))
    }
}
