//! Locate and clear the bridge-cache entries a documented run writes.
//!
//! The cache directory is named from a hash of the package files, the schema,
//! and the tool version, so a test cannot name the entry it wants. Every entry
//! records the application name it was generated for, which identifies the
//! fixture exactly. The metrics lane clears entries before a cold run, and the
//! publish lane pre-seeds and then finds the entry a later run must compare
//! against, so the lookup lives here rather than in either lane.

use anyhow::{Context, Result};
use cap_std::{
    ambient_authority,
    fs::{Dir, DirEntry},
};
use serde_json::Value;
use std::{
    ffi::OsStr,
    io::Read,
    path::{Path, PathBuf},
};

/// The directory Cargo's target holds the bridge cache below.
const CACHE_ROOT: &str = "orthohelp";

/// The intermediate representation's file name within a cache entry.
pub(super) const IR_FILE: &str = "ir.json";

/// Delete the bridge-cache entries generated for one application.
///
/// Other scenarios share this target and match a different name, so their
/// entries must survive; a missing cache root simply means the next run is
/// cold for every package.
pub(super) fn clear_cached_ir(target: &Path, app_name: &str) -> Result<()> {
    let Some(cache) = open_cache_root(target)? else {
        return Ok(());
    };
    for candidate in cache.entries()? {
        remove_fixture_entry(&cache, &candidate?, app_name)?;
    }
    Ok(())
}

/// The `ir.json` of the cache entry generated for `app_name`.
///
/// Returns `None` when the cache root or a matching entry is absent. An entry
/// that cannot be read or parsed is not evidence about the fixture, so it does
/// not match; the caller reports the absence rather than guessing.
pub(super) fn find_fixture_ir(target: &Path, app_name: &str) -> Result<Option<PathBuf>> {
    let Some(cache) = open_cache_root(target)? else {
        return Ok(None);
    };
    for candidate in cache.entries()? {
        let entry = candidate?;
        let name = entry.file_name();
        if entry.file_type()?.is_dir() && is_fixture_ir(&cache, &name, app_name) {
            return Ok(Some(target.join(CACHE_ROOT).join(&name).join(IR_FILE)));
        }
    }
    Ok(None)
}

/// Open the cache root, treating an absent root as an already-cold cache.
///
/// Only absence means "nothing cached". Any other open failure is reported:
/// folding it into an empty cache would let an unreadable target directory look
/// like a cache this test had already cleared, and the cold-run assertions
/// would then be checking a state nothing established.
fn open_cache_root(target: &Path) -> Result<Option<Dir>> {
    let cache_root = target.join(CACHE_ROOT);
    let opened = Dir::open_ambient_dir(&cache_root, ambient_authority());
    if opened.as_ref().err().map(std::io::Error::kind) == Some(std::io::ErrorKind::NotFound) {
        return Ok(None);
    }
    let cache = opened.with_context(|| format!("open {}", cache_root.display()))?;
    Ok(Some(cache))
}

/// Remove one cache entry when it holds IR generated for `app_name`.
///
/// Entries this repository did not write — another application's, a partial
/// publish, unreadable IR — are not ours to delete, so they stay.
fn remove_fixture_entry(cache: &Dir, entry: &DirEntry, app_name: &str) -> Result<()> {
    let name = entry.file_name();
    if entry.file_type()?.is_dir() && is_fixture_ir(cache, &name, app_name) {
        cache
            .remove_dir_all(&name)
            .with_context(|| format!("remove cached IR entry {}", name.to_string_lossy()))?;
    }
    Ok(())
}

/// Report whether a cache entry records `app_name` as the application it was
/// generated for.
///
/// An entry that cannot be read or parsed as JSON is not evidence about the
/// fixture, so it counts as another application's and is left alone.
fn is_fixture_ir(cache: &Dir, name: &OsStr, app_name: &str) -> bool {
    let Ok(directory) = cache.open_dir(name) else {
        return false;
    };
    let Ok(Some(ir_text)) = read_entry_ir(&directory) else {
        return false;
    };
    let Ok(ir) = serde_json::from_str::<Value>(&ir_text) else {
        return false;
    };
    ir.get("app_name").and_then(Value::as_str) == Some(app_name)
}

/// Read an entry's IR, or report that it is not a regular file.
///
/// The walk runs before the command deadline starts and holds the
/// documented-run lock, so a FIFO left behind by a test that terminated
/// without unwinding must not be read: a blocking open would hang the next
/// documented run. The opened handle is inspected before any read, which is
/// also what keeps the identity match to regular files only.
fn read_entry_ir(directory: &Dir) -> std::io::Result<Option<String>> {
    let mut file = open_entry_file(directory)?;
    if !file.metadata()?.is_file() {
        return Ok(None);
    }
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(Some(text))
}

/// Open an entry's IR without waiting for a writer when it is a FIFO.
#[cfg(unix)]
fn open_entry_file(directory: &Dir) -> std::io::Result<cap_std::fs::File> {
    use cap_std::fs::OpenOptionsExt;

    let nonblock = i32::try_from(rustix::fs::OFlags::NONBLOCK.bits())
        .map_err(|_| std::io::Error::other("O_NONBLOCK does not fit in the open flags word"))?;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).custom_flags(nonblock);
    directory.open_with(IR_FILE, &options)
}

/// Open an entry's IR; only Unix distinguishes a FIFO before opening it.
#[cfg(not(unix))]
fn open_entry_file(directory: &Dir) -> std::io::Result<cap_std::fs::File> {
    directory.open(IR_FILE)
}
