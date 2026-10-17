//! Diagnostic emissions for the bridge build and its IR cache.
//!
//! Split from the parent `bridge` module for two reasons. The first is the
//! workspace's 400-line module cap, which the parent reaches once each event
//! spelling has its own function. The second is that a `tracing` event is
//! expanded, when a dependent crate enables `tracing`'s `log` bridge, into a
//! branch ladder that counts against the workspace's cognitive-complexity
//! ceiling — so each spelling must sit alone in a function, and those
//! functions belong together.
//!
//! Nothing here decides anything: every function reports a fact the caller has
//! already established, and no control flow returns through this module.

use super::BridgePaths;

/// Report a successful bridge build.
///
/// Each emission lives in its own function so `build_bridge` holds at most
/// one `tracing` call: when a dependent crate enables `tracing`'s `log`
/// bridge, Cargo's feature unification expands every event into a branch
/// ladder that counts against the workspace's cognitive-complexity ceiling.
pub(super) fn bridge_build_succeeded(paths: &BridgePaths, status: std::process::ExitStatus) {
    tracing::debug!(
        manifest = %paths.manifest_path,
        status = %status,
        "bridge build succeeded"
    );
}

/// Report a failed bridge build with the child process's standard error.
pub(super) fn bridge_build_failed(paths: &BridgePaths, output: &std::process::Output) {
    tracing::debug!(
        manifest = %paths.manifest_path,
        status = %output.status,
        stderr = %String::from_utf8_lossy(&output.stderr),
        "bridge build failed"
    );
}

/// Report that the cached IR already matches, so the write was skipped.
///
/// Each emission lives in its own function so `write_ir_cache` holds at most
/// one `tracing` call: when a dependent crate enables `tracing`'s `log`
/// bridge, Cargo's feature unification expands every event into a branch
/// ladder that counts against the workspace's cognitive-complexity ceiling.
pub(super) fn ir_cache_unchanged(paths: &BridgePaths) {
    tracing::debug!(
        path = %paths.ir_path,
        "skipping ir.json write: cached content is identical"
    );
}

/// Report that the IR cache is about to be written, with its size.
pub(super) fn ir_cache_write(paths: &BridgePaths, bytes: usize) {
    tracing::debug!(
        path = %paths.ir_path,
        bytes,
        "writing ir.json cache"
    );
}
