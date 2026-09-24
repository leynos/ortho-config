//! Configuration-file fixtures for the scoped-discovery test suites.
//!
//! Split from `scoped_layers.rs` so each suite stays within the repository's
//! 400-line code file ceiling. Each test binary compiles its own copy of this
//! module, so every helper here must be one both suites use.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Write `body` to `path`, creating parent directories as needed.
///
/// # Errors
///
/// Returns an error when a parent directory cannot be created or the file
/// cannot be written.
pub fn write_body(path: &Path, body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory for {}", parent.display()))?;
    }
    fs::write(path, body).with_context(|| format!("write config file {}", path.display()))?;
    Ok(())
}

/// Write a minimal `value = <value>` configuration fixture.
///
/// # Errors
///
/// Returns an error when the file cannot be written.
pub fn write_config(path: &Path, value: u32) -> Result<()> {
    write_body(path, &format!("value = {value}\n"))
}
