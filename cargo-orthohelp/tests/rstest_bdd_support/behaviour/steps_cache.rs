//! Cache assertion step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements the `then` steps that verify IR cache behaviour:
//! - **`the cached IR matches its recorded content`** — confirms the
//!   `ir.json` content is byte-identical to the content recorded before the
//!   relevant invocation.
//! - **`the cached IR deserializes into the schema`** — deserializes the
//!   cached file into [`ortho_config::docs::DocMetadata`] and asserts the
//!   `ir_version` field matches [`ORTHO_DOCS_IR_VERSION`].
//! - **`the command fails due to missing cache`** — asserts that
//!   `cargo-orthohelp --no-build` exits non-zero and writes `MissingCache` to
//!   stderr when no cache exists.
//! - **`the command fails with a cache I/O error`** — checks public cache-read
//!   and cache-write failures against an invalid temporary cache.
//!
//! The generic **`the command succeeds`** assertion is registered by
//! [`super::steps_policy`] and reused by the cache scenarios.
//!
//! The `when` steps also corrupt or replace the temporary cache file to drive
//! the public CLI through replacement and failure paths.
//!
//! Also exposes [`CACHE_ARGS`] (the standard `--cache` argument slice) and
//! [`record_cache_state`] (reads the current `ir.json` path and content for
//! later comparison). Both are used by [`super::steps_cmd`].

use std::io::{Read, Write};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, DirEntry, OpenOptions};
use cap_std::time::SystemTime;
use rstest_bdd_macros::{then, when};

use super::steps::{OrthoHelpContext, StepResult, scenario_target_dir};

/// CLI arguments for cache-mode invocations of `cargo-orthohelp` under the
/// fixture package and the `en-US` / `fr-FR` locales.
pub(super) const CACHE_ARGS: &[&str] = &[
    "--cache",
    "--package",
    "orthohelp_fixture",
    "--locale",
    "en-US",
    "--locale",
    "fr-FR",
];

#[then("the cached IR matches its recorded content")]
fn cached_ir_matches_recorded_content(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let (dir, file_name) = recorded_cache_location(orthohelp_context)?;
    let previous_content = orthohelp_context
        .cache_ir_content
        .with_ref(Clone::clone)
        .ok_or("cached IR content should be recorded")?;
    let current_content = read_cache_entry(&dir, &file_name)?;
    assert_eq!(
        previous_content, current_content,
        "cached IR should match its recorded content"
    );
    Ok(())
}

#[then("the cached IR deserializes into the schema")]
fn cached_ir_deserializes(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = orthohelp_context
        .cache_ir_path
        .with_ref(Clone::clone)
        .ok_or("cached IR path should be recorded")?;
    let json = read_cache_file(&cache_path)?;
    let metadata: ortho_config::docs::DocMetadata = serde_json::from_str(&json)?;
    assert_eq!(
        metadata.ir_version,
        ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "cached IR should match the current schema version"
    );
    Ok(())
}

#[then("the command fails with a cache I/O error")]
fn command_fails_with_cache_io_error(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let failure = orthohelp_context
        .last_output
        .with_ref(|output| {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            (!output.status.success()).then_some(stderr)
        })
        .ok_or("last_output should be set")?
        .ok_or("cargo-orthohelp should fail for the invalid cache")?;
    assert!(
        failure.contains("Io"),
        "expected a cache I/O error, got: {failure}"
    );
    Ok(())
}

#[when("I corrupt the cached IR")]
fn corrupt_cached_ir(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let (dir, file_name) = recorded_cache_location(orthohelp_context)?;
    let mut file = dir.open_with(&file_name, OpenOptions::new().write(true).truncate(true))?;
    file.write_all(b"stale cache content")?;
    Ok(())
}

#[when("I replace the cached IR with a directory")]
fn replace_cached_ir_with_directory(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let (dir, file_name) = recorded_cache_location(orthohelp_context)?;
    dir.remove_file(&file_name)?;
    dir.create_dir(&file_name)?;
    Ok(())
}

#[when("I replace the cached IR directory with a read-only file")]
fn replace_cached_ir_directory_with_read_only_file(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let (dir, file_name) = recorded_cache_location(orthohelp_context)?;
    dir.remove_dir(&file_name)?;
    let mut file = dir.open_with(&file_name, OpenOptions::new().write(true).create_new(true))?;
    file.write_all(b"stale cache content")?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions)?;
    Ok(())
}

#[when("I restore cached IR file permissions")]
fn restore_cached_ir_permissions(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let (dir, file_name) = recorded_cache_location(orthohelp_context)?;
    let mut permissions = dir.metadata(&file_name)?.permissions();
    permissions.set_readonly(false);
    dir.set_permissions(&file_name, permissions)?;
    Ok(())
}

#[then("the command fails due to missing cache")]
// `main` returns `OrthohelpError` directly, so Rust prints the Debug variant.
// Keep this coupled to `MissingCache` to verify the intended failure path.
fn command_fails_due_to_missing_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let failed_with_missing_cache = orthohelp_context
        .last_output
        .with_ref(|output| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            !output.status.success() && stderr.contains("MissingCache")
        })
        .ok_or("last_output should be set")?;
    if !failed_with_missing_cache {
        return Err("expected command failure due to missing cache".into());
    }
    Ok(())
}

/// Reads the current cached `ir.json` path and content so that
/// `cached_ir_matches_recorded_content` can later verify the file contents.
pub(super) fn record_cache_state(ctx: &OrthoHelpContext) -> StepResult<(Utf8PathBuf, String)> {
    let cache_path = find_cached_ir(ctx)?.ok_or("cached IR should exist")?;
    // Read the full content: gives a stable reference for comparison and forces
    // Windows NTFS to commit the final mtime before any subsequent metadata read.
    let content = read_cache_file(&cache_path)?;
    Ok((cache_path, content))
}

fn read_cache_file(path: &Utf8Path) -> StepResult<String> {
    let (dir, file_name) = open_cache_parent(path)?;
    read_cache_entry(&dir, &file_name)
}

fn recorded_cache_location(ctx: &OrthoHelpContext) -> StepResult<(Dir, String)> {
    let cache_path = ctx
        .cache_ir_path
        .with_ref(Clone::clone)
        .ok_or("cached IR path should be recorded")?;
    open_cache_parent(&cache_path)
}

fn open_cache_parent(path: &Utf8Path) -> StepResult<(Dir, String)> {
    let cache_dir = path.parent().ok_or("cached IR parent missing")?;
    let file_name = path
        .file_name()
        .ok_or("cached IR filename missing")?
        .to_owned();
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())?;
    Ok((dir, file_name))
}

fn read_cache_entry(dir: &Dir, file_name: &str) -> StepResult<String> {
    let mut file = dir.open(file_name)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn find_cached_ir(ctx: &OrthoHelpContext) -> StepResult<Option<Utf8PathBuf>> {
    let target_dir = scenario_target_dir(ctx)?;
    let cache_root = target_dir.join("orthohelp");
    let dir = match Dir::open_ambient_dir(&cache_root, ambient_authority()) {
        Ok(d) => d,
        Err(e) if is_not_found_kind(&e) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut newest: Option<(SystemTime, Utf8PathBuf)> = None;
    for entry in dir.read_dir(".")? {
        if let Some(candidate) = check_cache_entry(&dir, &cache_root, entry, newest.as_ref()) {
            newest = Some(candidate);
        }
    }
    Ok(newest.map(|(_, path)| path))
}

fn check_cache_entry(
    dir: &Dir,
    cache_root: &Utf8PathBuf,
    entry_result: Result<DirEntry, std::io::Error>,
    newest: Option<&(SystemTime, Utf8PathBuf)>,
) -> Option<(SystemTime, Utf8PathBuf)> {
    let entry = entry_result.ok()?;
    let file_type = entry.file_type().ok()?;
    if !file_type.is_dir() {
        return None;
    }

    let file_name = entry.file_name().ok()?;
    let relative = Utf8PathBuf::from(file_name).join("ir.json");
    let metadata = dir.metadata(&relative).ok()?;
    let modified = metadata.modified().ok()?;
    let should_replace = newest.is_none_or(|(best_time, _)| modified > *best_time);
    if !should_replace {
        return None;
    }

    Some((modified, cache_root.join(relative)))
}

/// Returns `true` when `err` represents a "not found" I/O error, used to
/// treat a missing cache root as `Ok(None)` rather than a hard failure.
pub(super) fn is_not_found_kind(err: &std::io::Error) -> bool {
    matches!(err.kind(), std::io::ErrorKind::NotFound)
}
