//! Capability-based reads for the coverage gate.
//!
//! The workspace denies `std::fs` operations through the Whitaker Dylint
//! suite, so every read goes through a `cap_std` directory handle opened on the
//! file's parent. Keeping that one helper here means the gate and its negative
//! controls cannot quietly reintroduce an ambient path read.

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

/// Read `path` as UTF-8 text through a capability on its parent directory.
///
/// The gate uses it for the workspace lock and for each reviewed fixture:
///
/// ```ignore
/// let document = read_text(&metadata.workspace_root.join("Cargo.lock"))?;
/// ```
pub(crate) fn read_text(path: &Utf8Path) -> Result<String> {
    let parent = path
        .parent()
        .with_context(|| format!("{path} should have a parent directory"))?;
    let name = path
        .file_name()
        .with_context(|| format!("{path} should name a file"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .with_context(|| format!("open {parent} to read {name}"))?;
    directory
        .read_to_string(name)
        .with_context(|| format!("read {path}"))
}
