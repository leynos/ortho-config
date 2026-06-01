//! Shared helpers for `cargo-orthohelp` integration tests.
//!
//! These helpers hide process-launch plumbing from dispatch tests such as
//! `cli_dispatch.rs`: `cargo_orthohelp_exe` resolves the compiled binary across
//! Cargo and nextest environments, while `workspace_root` gives spawned
//! commands a stable repository working directory.

use camino::Utf8PathBuf;
use std::error::Error;

const ENV_VARS: &[&str] = &[
    "CARGO_BIN_EXE_cargo-orthohelp",
    "CARGO_BIN_EXE_cargo_orthohelp",
    "NEXTEST_BIN_EXE_cargo-orthohelp",
    "NEXTEST_BIN_EXE_cargo_orthohelp",
];

/// Resolves the compiled `cargo-orthohelp` binary path from test environment
/// variables.
///
/// # Errors
///
/// Returns an error when none of the supported cargo/nextest binary
/// environment variables are present.
pub(crate) fn cargo_orthohelp_exe() -> Result<Utf8PathBuf, Box<dyn Error + Send + Sync>> {
    if let Some(path) = compile_time_binary_paths().into_iter().flatten().next() {
        return Ok(Utf8PathBuf::from(path));
    }

    if let Some(path) = ENV_VARS.iter().find_map(|var| std::env::var(var).ok()) {
        return Ok(Utf8PathBuf::from(path));
    }

    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let debug_bin = manifest_dir
        .parent()
        .map(|root| root.join("target").join("debug").join("cargo-orthohelp"))
        .filter(|p| p.exists());
    if let Some(path) = debug_bin {
        return Ok(path);
    }

    Err("cargo-orthohelp binary path not found in environment".into())
}

/// Resolves the repository workspace root from the crate manifest directory.
///
/// # Errors
///
/// Returns an error when `CARGO_MANIFEST_DIR` has no parent directory.
pub(crate) fn workspace_root() -> Result<Utf8PathBuf, Box<dyn Error + Send + Sync>> {
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
