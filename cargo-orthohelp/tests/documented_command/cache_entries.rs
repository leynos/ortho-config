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
    let Ok(ir_text) = directory.read_to_string(IR_FILE) else {
        return false;
    };
    let Ok(ir) = serde_json::from_str::<Value>(&ir_text) else {
        return false;
    };
    ir.get("app_name").and_then(Value::as_str) == Some(app_name)
}
