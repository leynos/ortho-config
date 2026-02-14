//! Shared helpers for `cargo-orthohelp` integration tests.

use camino::Utf8PathBuf;
use std::error::Error;

/// Resolves the compiled `cargo-orthohelp` binary path from test environment
/// variables.
///
/// # Errors
///
/// Returns an error when none of the supported cargo/nextest binary
/// environment variables are present.
pub(crate) fn cargo_orthohelp_exe() -> Result<Utf8PathBuf, Box<dyn Error>> {
    let env_vars = [
        "CARGO_BIN_EXE_cargo-orthohelp",
        "CARGO_BIN_EXE_cargo_orthohelp",
        "NEXTEST_BIN_EXE_cargo-orthohelp",
        "NEXTEST_BIN_EXE_cargo_orthohelp",
    ];
    for var in env_vars {
        if let Ok(path) = std::env::var(var) {
            return Ok(Utf8PathBuf::from(path));
        }
    }
    Err("cargo-orthohelp binary path not found in environment".into())
}
