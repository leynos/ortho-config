//! Cache assertion helpers and step definitions for `cargo-orthohelp`.

use std::io::Read;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, DirEntry};
use cap_std::time::SystemTime;
use rstest_bdd_macros::then;

use super::steps::{OrthoHelpContext, StepResult};
use super::steps_cmd::scenario_target_dir;

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

#[then("the cached IR is reused")]
fn cached_ir_reused(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = orthohelp_context
        .cache_ir_path
        .with_ref(Clone::clone)
        .ok_or("cached IR path should be recorded")?;
    let previous_content = orthohelp_context
        .cache_ir_content
        .with_ref(Clone::clone)
        .ok_or("cached IR content should be recorded")?;
    let current_content = read_cache_file(&cache_path)?;
    assert_eq!(
        previous_content, current_content,
        "cached IR should not be rewritten"
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

#[then("the command fails due to missing cache")]
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

/// Records the current state of the cached `ir.json` file into `ctx` so that
/// [`cached_ir_reused`] can later verify no rewrite occurred.
pub(super) fn record_cache_state(ctx: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = find_cached_ir(ctx)?.ok_or("cached IR should exist")?;
    // Read the full content: gives a stable reference for comparison and forces
    // Windows NTFS to commit the final mtime before any subsequent metadata read.
    let content = read_cache_file(&cache_path)?;
    ctx.cache_ir_path.set(cache_path);
    ctx.cache_ir_content.set(content);
    Ok(())
}

fn read_cache_file(path: &Utf8Path) -> StepResult<String> {
    let cache_dir = path.parent().ok_or("cached IR parent missing")?;
    let file_name = path.file_name().ok_or("cached IR filename missing")?;
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())?;
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
        if let Some(candidate) = check_cache_entry(&cache_root, entry, newest.as_ref()) {
            newest = Some(candidate);
        }
    }
    Ok(newest.map(|(_, path)| path))
}

fn check_cache_entry(
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
    let dir = Dir::open_ambient_dir(cache_root.as_path(), ambient_authority()).ok()?;
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
