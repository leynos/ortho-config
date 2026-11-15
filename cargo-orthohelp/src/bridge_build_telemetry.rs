//! Diagnostic emissions for the bridge Cargo build.
//!
//! Each emission lives in its own function. When a dependent crate enables
//! `tracing`'s `log` bridge, Cargo's feature unification expands every event
//! into a branch ladder that counts against the workspace's
//! cognitive-complexity ceiling, so the function that *decides* something must
//! not also spell the event. Keeping the spellings together here mirrors the
//! bridge IR-cache telemetry module.
//!
//! Nothing here decides anything: every function reports a fact the caller has
//! already established, and no control flow returns through this module.

/// Report that a stale recovery marker blocked reuse of a Cargo target.
pub(super) fn stale_marker_rejected() {
    tracing::warn!(
        operation = "bridge_build_marker",
        outcome = "stale_marker_rejected",
        marker_retained = true,
        "bridge build requires owner recovery after an interrupted build"
    );
}

/// Report that a failed build retained its marker because a writer may live on.
pub(super) fn writer_lifetime_unconfirmed() {
    tracing::warn!(
        operation = "bridge_build_and_snapshot",
        outcome = "writer_lifetime_unconfirmed",
        marker_retained = true,
        "bridge child lifetime is unconfirmed; recovery marker retained"
    );
}

/// Report that a generated bridge output could not be opened for removal.
pub(super) fn cleanup_target_open_failed() {
    tracing::warn!(
        operation = "bridge_output_cleanup",
        outcome = "target_open_failed",
        "could not open Cargo target to remove generated bridge outputs"
    );
}

/// Report that a generated bridge output resolved outside its Cargo target.
pub(super) fn cleanup_output_outside_target() {
    tracing::warn!(
        operation = "bridge_output_cleanup",
        outcome = "outside_target_ignored",
        "ignored generated bridge output outside its Cargo target"
    );
}

/// Report that a generated bridge output survived removal.
pub(super) fn cleanup_remove_failed() {
    tracing::warn!(
        operation = "bridge_output_cleanup",
        outcome = "remove_failed",
        "failed to remove generated bridge Cargo output after snapshot"
    );
}

/// Report the outcome of removing generated bridge outputs after a snapshot.
pub(super) fn cleanup_completed(outcome: &'static str) {
    tracing::debug!(
        operation = "bridge_output_cleanup",
        outcome,
        "generated bridge output cleanup completed"
    );
}

/// Report that an owned Cargo child resisted termination on scope exit.
pub(super) fn owned_child_kill_failed(error_kind: std::io::ErrorKind) {
    tracing::warn!(
        error_kind = ?error_kind,
        "failed to terminate owned Cargo child"
    );
}

/// Report that a forwarded Cargo setting was withheld as credential-bearing.
///
/// Also reachable from the environment policy, which sits beside this module
/// rather than under it.
pub(in crate::bridge) fn credential_bearing_setting_omitted(variable: &str) {
    tracing::warn!(
        variable,
        "omitted credential-bearing Cargo build setting; configure proxy and registry credentials in Cargo configuration under CARGO_HOME"
    );
}
