//! Enables Cargo to provide `OUT_DIR` to opt-in proc-macro artefact tests.

/// Build script entry point; it only has to exist so Cargo sets `OUT_DIR`.
///
/// The identifier artefact tests read the emitted JSON from that directory.
fn main() {}
