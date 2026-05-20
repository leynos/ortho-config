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
    if let Some(path) = compile_time_binary_paths().into_iter().flatten().next() {
        return Ok(Utf8PathBuf::from(path));
    }

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

/// Resolves the repository workspace root from the crate manifest directory.
///
/// # Errors
///
/// Returns an error when `CARGO_MANIFEST_DIR` has no parent directory.
pub(crate) fn workspace_root() -> Result<Utf8PathBuf, Box<dyn Error>> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .parent()
        .ok_or_else(|| format!("workspace root should exist above {manifest_dir}"))?
        .to_path_buf())
}

const fn compile_time_binary_paths() -> [Option<&'static str>; 2] {
    [
        option_env!("CARGO_BIN_EXE_cargo-orthohelp"),
        option_env!("CARGO_BIN_EXE_cargo_orthohelp"),
    ]
}
