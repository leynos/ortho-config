//! Resolving the workspace target Cargo has configured.
//!
//! Boundary fixtures live below `target/`, but their compiled dependencies
//! reuse the workspace target. This keeps a foreign package's sources and
//! manifests separate while reusing dependencies the parent gate has already
//! compiled.

use super::environment::CARGO_DEADLINE;
use super::{cargo_binary, checked_stdout, inherited_toolchain, run_with_deadline};
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static WORKSPACE_TARGET_DIRECTORY: OnceLock<Result<Utf8PathBuf, String>> = OnceLock::new();

/// Return the target directory Cargo resolves for the workspace.
///
/// `cargo metadata` reports the configured target, including a caller's
/// `CARGO_TARGET_DIR` override. One test binary belongs to one workspace, so
/// cache that discovery and use the same target for package verification and
/// its foreign consumers.
///
/// # Errors
///
/// Returns an error when Cargo metadata cannot be run, fails, or omits the
/// workspace target directory.
///
/// # Examples
///
/// The path comes from Cargo's own workspace resolution, including a
/// configured target override:
///
/// ```text
/// cargo metadata --offline --no-deps --format-version=1
/// {"target_directory":"/workspace/target", ...}
/// ```
pub(crate) fn workspace_target_directory(repository: &Utf8Path) -> Result<Utf8PathBuf> {
    let cached = WORKSPACE_TARGET_DIRECTORY
        .get_or_init(|| discover(repository).map_err(|error| format!("{error:#}")));
    cached
        .as_ref()
        .cloned()
        .map_err(|message| anyhow::anyhow!(message.clone()))
}

/// Run Cargo metadata once and parse its selected target directory.
fn discover(repository: &Utf8Path) -> Result<Utf8PathBuf> {
    let mut command = Command::new(cargo_binary());
    command
        .current_dir(repository.as_std_path())
        .args(["metadata", "--offline", "--no-deps", "--format-version=1"])
        .env_clear()
        .envs(inherited_toolchain());
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        command.env("CARGO_TARGET_DIR", target);
    }
    let output = run_with_deadline(command, CARGO_DEADLINE, "cargo metadata target discovery")?;
    let stdout = checked_stdout(&output, "cargo metadata target discovery")?;
    parse_target_directory(&stdout)
}

/// Read Cargo's `target_directory` field from metadata JSON.
///
/// # Examples
///
/// ```text
/// {"target_directory":"/workspace/target"}
/// -> /workspace/target
/// ```
fn parse_target_directory(stdout: &str) -> Result<Utf8PathBuf> {
    let document: serde_json::Value = serde_json::from_str(stdout)
        .context("parse workspace metadata for its target directory")?;
    let target = document
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .context("Cargo metadata omitted target_directory")?;
    Ok(Utf8PathBuf::from(target))
}

#[cfg(test)]
mod tests {
    //! Parser tests for Cargo's resolved workspace target.

    use super::parse_target_directory;

    #[test]
    fn reads_the_target_directory_reported_by_cargo() {
        let target = parse_target_directory(r#"{"target_directory":"/workspace/build"}"#)
            .expect("metadata with a target_directory is valid");
        assert_eq!(target.as_str(), "/workspace/build");
    }

    #[test]
    fn refuses_metadata_without_a_target_directory() {
        let result = parse_target_directory(r#"{"packages":[]}"#);
        assert!(result.is_err());
    }
}
