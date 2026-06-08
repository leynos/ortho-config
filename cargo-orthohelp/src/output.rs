//! Output writers for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;
use ortho_config::AgentContext;

static AGENT_CONTEXT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes the localized IR JSON for a single locale.
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
pub fn write_agent_context(
    out_dir: &Utf8Path,
    payload: &AgentContext,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let target = AgentContextWriteTarget::new(out_dir);
    let dir = ensure_dir(out_dir).map_err(|error| {
        tracing::debug!(
            path = %out_dir,
            error = %error,
            "failed to prepare agent-context output directory",
        );
        error
    })?;
    let mut file = open_agent_context_temp_file(&dir, &target.temp_filename, &target.temp_path)?;

    let content = serde_json::to_string_pretty(payload).map_err(|error| {
        tracing::debug!(
            path = %target.path,
            error = %error,
            "failed to serialize agent-context JSON",
        );
        OrthohelpError::IrJson(error)
    })?;
    tracing::debug!(
        path = %target.path,
        bytes = content.len(),
        "writing agent-context JSON",
    );
    write_and_sync_agent_context_temp_file(&mut file, &target.temp_path, &content)?;
    drop(file);

    replace_agent_context_file(&dir, &target)?;
    sync_parent_dir(&dir, out_dir).map_err(|error| {
        tracing::debug!(
            path = %out_dir,
            error = %error,
            "failed to sync agent-context output directory",
        );
        error
    })?;
    tracing::debug!(
        path = %target.path,
        bytes = content.len(),
        "agent-context JSON written successfully",
    );

    Ok(target.path)
}

struct AgentContextWriteTarget {
    filename: &'static str,
    path: Utf8PathBuf,
    temp_filename: String,
    temp_path: Utf8PathBuf,
}

impl AgentContextWriteTarget {
    fn new(out_dir: &Utf8Path) -> Self {
        let filename = "agent-context.json";
        let temp_id = AGENT_CONTEXT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_filename = format!("{filename}.{}.{}.tmp", std::process::id(), temp_id);
        Self {
            filename,
            path: out_dir.join(filename),
            temp_path: out_dir.join(&temp_filename),
            temp_filename,
        }
    }
}

fn open_agent_context_temp_file(
    dir: &Dir,
    temp_filename: &str,
    temp_target: &Utf8Path,
) -> Result<File, OrthohelpError> {
    dir.open_with(
        temp_filename,
        OpenOptions::new().write(true).create(true).truncate(true),
    )
    .map_err(|io_err| {
        tracing::debug!(
            path = %temp_target,
            error = %io_err,
            "failed to open agent-context JSON for writing",
        );
        OrthohelpError::Io {
            path: temp_target.to_path_buf(),
            source: io_err,
        }
    })
}

fn write_and_sync_agent_context_temp_file(
    file: &mut File,
    temp_target: &Utf8Path,
    content: &str,
) -> Result<(), OrthohelpError> {
    file.write_all(content.as_bytes()).map_err(|io_err| {
        tracing::debug!(
            path = %temp_target,
            bytes = content.len(),
            error = %io_err,
            "failed to write agent-context JSON",
        );
        OrthohelpError::Io {
            path: temp_target.to_path_buf(),
            source: io_err,
        }
    })?;
    file.flush().map_err(|io_err| {
        tracing::debug!(
            path = %temp_target,
            bytes = content.len(),
            error = %io_err,
            "failed to flush agent-context JSON",
        );
        OrthohelpError::Io {
            path: temp_target.to_path_buf(),
            source: io_err,
        }
    })?;
    file.sync_all().map_err(|io_err| {
        tracing::debug!(
            path = %temp_target,
            bytes = content.len(),
            error = %io_err,
            "failed to sync agent-context JSON",
        );
        OrthohelpError::Io {
            path: temp_target.to_path_buf(),
            source: io_err,
        }
    })
}

fn replace_agent_context_file(
    dir: &Dir,
    target: &AgentContextWriteTarget,
) -> Result<(), OrthohelpError> {
    dir.rename(&target.temp_filename, dir, target.filename)
        .map_err(|io_err| {
            tracing::debug!(
                source = %target.temp_path,
                destination = %target.path,
                error = %io_err,
                "failed to replace agent-context JSON",
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
