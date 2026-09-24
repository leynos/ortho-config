//! Output writers for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;
use crate::policy::PolicyReport;
use ortho_config::AgentContext;

// Process-wide suffix for atomic JSON artefact temp names. `Relaxed` ordering
// hands out distinct values; the `create_new` and rename operations provide
// the actual synchronization, so any collision is a hard failure.
static JSON_ARTEFACT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes the localized IR JSON for a single locale.
///
/// # Errors
/// Returns an I/O error when preparing or writing the file, or a JSON error
/// on serialization.
pub fn write_localized_ir(
    out_dir: &Utf8Path,
    locale: &str,
    payload: &LocalizedDocMetadata,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let dir = ensure_dir(out_dir)?;
    dir.create_dir_all("ir")
        .map_err(|io_err| OrthohelpError::Io {
            path: out_dir.to_path_buf(),
            source: io_err,
        })?;

    let ir_dir = out_dir.join("ir");
    let ir_dir_handle = dir.open_dir("ir").map_err(|io_err| OrthohelpError::Io {
        path: ir_dir.clone(),
        source: io_err,
    })?;
    let filename = format!("{locale}.json");
    let mut file = ir_dir_handle
        .open_with(
            &filename,
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: ir_dir.join(&filename),
            source: io_err,
        })?;

    let content = serde_json::to_string_pretty(payload)?;
    file.write_all(content.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: ir_dir.join(&filename),
            source: io_err,
        })?;

    Ok(ir_dir.join(filename))
}

/// Writes the compact agent-context JSON document.
///
/// # Errors
/// Returns a JSON error on serialization or an I/O error on the atomic write.
pub fn write_agent_context(
    out_dir: &Utf8Path,
    payload: &AgentContext,
) -> Result<Utf8PathBuf, OrthohelpError> {
    tracing::debug!(
        package = %payload.package,
        out_dir = %out_dir,
        "starting agent-context write",
    );
    let content = serde_json::to_string_pretty(payload).map_err(|error| {
        tracing::debug!(
            package = %payload.package,
            path = %out_dir,
            error = %error,
            "failed to serialize agent-context JSON",
        );
        OrthohelpError::IrJson(error)
    })?;
    let path = write_atomic_json(out_dir, "agent-context.json", &content, "agent-context")?;
    tracing::debug!(
        package = %payload.package,
        path = %path,
        bytes = content.len(),
        "agent-context JSON written successfully",
    );
    Ok(path)
}

/// Writes the machine-readable policy-report JSON document.
///
/// # Errors
/// Returns a JSON error on serialization or an I/O error on the atomic write.
pub fn write_policy_report(
    out_dir: &Utf8Path,
    payload: &PolicyReport,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let content = serde_json::to_string_pretty(payload).map_err(OrthohelpError::IrJson)?;
    write_atomic_json(out_dir, "policy-report.json", &content, "policy-report")
}

/// Atomically writes one JSON artefact via temp file, rename, and fsync,
/// satisfying Decision D5's atomicity requirement for `policy-report.json`.
fn write_atomic_json(
    out_dir: &Utf8Path,
    filename: &'static str,
    content: &str,
    artefact: &str,
) -> Result<Utf8PathBuf, OrthohelpError> {
    tracing::debug!(artefact, out_dir = %out_dir, "starting atomic JSON write");
    let target = JsonArtefactWriteTarget::new(out_dir, filename);
    let dir = ensure_dir(out_dir).map_err(|error| {
        tracing::debug!(
            artefact,
            path = %out_dir,
            error = %error,
            "failed to prepare JSON output directory",
        );
        error
    })?;
    let mut file = open_json_temp_file(&dir, &target, artefact)?;
    write_and_sync_json_temp_file(&mut file, &target, content, artefact)?;
    drop(file);

    replace_json_file(&dir, &target, artefact)?;
    sync_parent_dir(&dir, out_dir).map_err(|error| {
        tracing::debug!(
            artefact,
            path = %out_dir,
            error = %error,
            "failed to sync JSON output directory",
        );
        error
    })?;
    tracing::debug!(artefact, path = %target.path, bytes = content.len(), "atomic JSON write complete");

    Ok(target.path)
}

struct JsonArtefactWriteTarget {
    filename: &'static str,
    path: Utf8PathBuf,
    temp_filename: String,
    temp_path: Utf8PathBuf,
}

impl JsonArtefactWriteTarget {
    fn new(out_dir: &Utf8Path, filename: &'static str) -> Self {
        let temp_id = JSON_ARTEFACT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_filename = format!("{filename}.{}.{}.tmp", std::process::id(), temp_id);
        Self {
            filename,
            path: out_dir.join(filename),
            temp_path: out_dir.join(&temp_filename),
            temp_filename,
        }
    }
}

fn open_json_temp_file(
    dir: &Dir,
    target: &JsonArtefactWriteTarget,
    artefact: &str,
) -> Result<File, OrthohelpError> {
    dir.open_with(
        &target.temp_filename,
        OpenOptions::new().write(true).create_new(true),
    )
    .map_err(|io_err| {
        tracing::debug!(
            artefact,
            path = %target.temp_path,
            error = %io_err,
            "failed to open JSON artefact for writing",
        );
        OrthohelpError::Io {
            path: target.temp_path.to_path_buf(),
            source: io_err,
        }
    })
}

fn write_and_sync_json_temp_file(
    file: &mut File,
    target: &JsonArtefactWriteTarget,
    content: &str,
    artefact: &str,
) -> Result<(), OrthohelpError> {
    file.write_all(content.as_bytes()).map_err(|io_err| {
        tracing::debug!(
            artefact,
            path = %target.temp_path,
            bytes = content.len(),
            error = %io_err,
            "failed to write JSON artefact",
        );
        OrthohelpError::Io {
            path: target.temp_path.to_path_buf(),
            source: io_err,
        }
    })?;
    file.flush().map_err(|io_err| {
        tracing::debug!(
            artefact,
            path = %target.temp_path,
            bytes = content.len(),
            error = %io_err,
            "failed to flush JSON artefact",
        );
        OrthohelpError::Io {
            path: target.temp_path.to_path_buf(),
            source: io_err,
        }
    })?;
    file.sync_all().map_err(|io_err| {
        tracing::debug!(
            artefact,
            path = %target.temp_path,
            bytes = content.len(),
            error = %io_err,
            "failed to sync JSON artefact",
        );
        OrthohelpError::Io {
            path: target.temp_path.to_path_buf(),
            source: io_err,
        }
    })
}

fn replace_json_file(
    dir: &Dir,
    target: &JsonArtefactWriteTarget,
    artefact: &str,
) -> Result<(), OrthohelpError> {
    dir.rename(&target.temp_filename, dir, target.filename)
        .map_err(|io_err| {
            tracing::debug!(
                artefact,
                source = %target.temp_path,
                destination = %target.path,
                error = %io_err,
                "failed to replace JSON artefact",
            );
            OrthohelpError::Io {
                path: target.path.clone(),
                source: io_err,
            }
        })
}

#[cfg(unix)]
fn sync_parent_dir(dir: &Dir, path: &Utf8Path) -> Result<(), OrthohelpError> {
    let dir_file = dir.open(".").map_err(|io_err| OrthohelpError::Io {
        path: path.to_path_buf(),
        source: io_err,
    })?;
    dir_file.sync_all().map_err(|io_err| OrthohelpError::Io {
        path: path.to_path_buf(),
        source: io_err,
    })
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "non-Unix stub mirrors the Unix fn signature so call sites \
              compile uniformly across platforms"
)]
#[cfg(not(unix))]
const fn sync_parent_dir(_dir: &Dir, _path: &Utf8Path) -> Result<(), OrthohelpError> {
    Ok(())
}

fn ensure_dir(path: &Utf8Path) -> Result<Dir, OrthohelpError> {
    match Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(dir) => Ok(dir),
        Err(open_err) if open_err.kind() == std::io::ErrorKind::NotFound => {
            Dir::create_ambient_dir_all(path, ambient_authority()).map_err(|io_err| {
                OrthohelpError::Io {
                    path: path.to_path_buf(),
                    source: io_err,
                }
            })?;
            Dir::open_ambient_dir(path, ambient_authority()).map_err(|io_err| OrthohelpError::Io {
                path: path.to_path_buf(),
                source: io_err,
            })
        }
        Err(open_err) => Err(OrthohelpError::Io {
            path: path.to_path_buf(),
            source: open_err,
        }),
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
