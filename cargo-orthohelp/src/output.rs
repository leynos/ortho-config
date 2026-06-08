//! Output writers for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::io::Write;

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;
use ortho_config::AgentContext;

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
    let filename = "agent-context.json";
    let target = out_dir.join(filename);
    let dir = ensure_dir(out_dir).map_err(|error| {
        tracing::debug!(
            path = %out_dir,
            error = %error,
            "failed to prepare agent-context output directory",
        );
        error
    })?;
    let mut file = dir
        .open_with(
            filename,
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| {
            tracing::debug!(
                path = %target,
                error = %io_err,
                "failed to open agent-context JSON for writing",
            );
            OrthohelpError::Io {
                path: target.clone(),
                source: io_err,
            }
        })?;

    let content = serde_json::to_string_pretty(payload).map_err(|error| {
        tracing::debug!(
            path = %target,
            error = %error,
            "failed to serialize agent-context JSON",
        );
        OrthohelpError::IrJson(error)
    })?;
    tracing::debug!(
        path = %target,
        bytes = content.len(),
        "writing agent-context JSON",
    );
    file.write_all(content.as_bytes()).map_err(|io_err| {
        tracing::debug!(
            path = %target,
            bytes = content.len(),
            error = %io_err,
            "failed to write agent-context JSON",
        );
        OrthohelpError::Io {
            path: target.clone(),
            source: io_err,
        }
    })?;
    tracing::debug!(
        path = %target,
        bytes = content.len(),
        "agent-context JSON written successfully",
    );

    Ok(target)
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
