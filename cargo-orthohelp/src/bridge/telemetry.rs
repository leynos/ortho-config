//! Diagnostic emissions for the bridge IR cache.
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
//!
//! The bridge *build* reports through its own modules: a failed build carries
//! child output that `bridge_build` already owns, so it has no fact to
//! contribute here.

use super::BridgeCache;

/// Report that the cached IR already matches, so the write was skipped.
///
/// Each emission lives in its own function so `write_ir_cache` holds at most
/// one `tracing` call: when a dependent crate enables `tracing`'s `log`
/// bridge, Cargo's feature unification expands every event into a branch
/// ladder that counts against the workspace's cognitive-complexity ceiling.
pub(super) fn ir_cache_unchanged(paths: &BridgeCache) {
    tracing::debug!(
        path = %paths.ir_path,
        "skipping ir.json write: cached content is identical"
    );
}

/// Report that the IR cache is about to be written, with its size.
pub(super) fn ir_cache_write(paths: &BridgeCache, bytes: usize) {
    tracing::debug!(
        path = %paths.ir_path,
        bytes,
        "writing ir.json cache"
    );
}
