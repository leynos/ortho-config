//! Configuration-file fixtures for the scoped-discovery test suites.
//!
//! Split from `scoped_layers.rs` so each suite stays within the repository's
//! 400-line code file ceiling. Each test binary compiles its own copy of this
//! module, so every helper here must be one both suites use.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use cap_std::{ambient_authority, fs::Dir};

/// Write `body` to `path`, creating parent directories as needed.
///
/// The write goes through a `cap_std::fs::Dir` handle rather than `std::fs`,
/// per the repository's filesystem policy: the handle names the directory the
/// helper may touch, instead of reaching into the ambient working directory.
/// The handle is opened on the deepest ancestor that already exists, and the
/// remaining components are created relative to it, so every directory this
/// touches stays beneath a root the caller supplied.
///
/// # Errors
///
/// Returns an error when the path has no parent directory or no UTF-8 file
/// name, when no ancestor directory exists, or when a parent cannot be created
/// or the file cannot be written.
pub fn write_body(path: &Path, body: &str) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("fixture path {} has no parent directory", path.display()))?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("fixture file name is not UTF-8: {}", path.display()))?;

    let (dir, relative) = open_deepest_existing_ancestor(parent)?;
    if !relative.as_os_str().is_empty() {
        dir.create_dir_all(&relative)
            .with_context(|| format!("create fixture directory {}", parent.display()))?;
    }
    // `relative` names `parent` from inside the handle, so the file must be
    // written below it; writing `name` alone would land beside the directory
    // just created rather than inside it.
    let target = relative.join(name);
    dir.write(&target, body.as_bytes())
        .with_context(|| format!("write config file {}", path.display()))?;
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

/// Open `dir`, or its nearest ancestor that exists, plus the remainder.
///
/// Returning the remainder rather than the original path is what lets the
/// caller create the missing components through the same handle: the relative
/// path is guaranteed to be a descendant of the handle's root.
fn open_deepest_existing_ancestor(dir: &Path) -> Result<(Dir, PathBuf)> {
    let mut candidate = dir;
    loop {
        if let Ok(handle) = Dir::open_ambient_dir(candidate, ambient_authority()) {
            let relative = dir
                .strip_prefix(candidate)
                .unwrap_or_else(|_| Path::new(""));
            return Ok((handle, relative.to_path_buf()));
        }
        candidate = candidate.parent().ok_or_else(|| {
            anyhow!(
                "no existing ancestor directory for fixture path {}",
                dir.display()
            )
        })?;
    }
}
