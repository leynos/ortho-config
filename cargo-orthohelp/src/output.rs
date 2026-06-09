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
    tracing::debug!(
        out_dir = %out_dir,
        "starting agent-context write",
    );
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
        OpenOptions::new().write(true).create_new(true),
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
mod tests {
    use super::*;
    use camino::Utf8Path;
    use std::collections::HashSet;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn concurrent_writes_do_not_corrupt_output() {
        let temp_dir = TempDir::new().expect("create temporary output directory");
        let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");
        let payload = Arc::new(AgentContext::new("test-package"));

        let handles = (0..8)
            .map(|_| {
                let thread_payload = Arc::clone(&payload);
                let thread_out_dir = out_dir.to_path_buf();
                std::thread::spawn(move || write_agent_context(&thread_out_dir, &thread_payload))
            })
            .collect::<Vec<_>>();

        for handle in handles {
            let result = handle.join().expect("thread panicked");
            assert!(result.is_ok(), "write_agent_context failed: {result:?}");
        }

        let content =
            std::fs::read_to_string(out_dir.join("agent-context.json")).expect("read output JSON");
        serde_json::from_str::<serde_json::Value>(&content).expect("parse output JSON");
    }

    #[test]
    fn temp_file_collision_fails_hard() {
        let temp_dir = TempDir::new().expect("create temporary output directory");
        let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");
        let dir = ensure_dir(out_dir).expect("open output directory");
        let temp_filename = "agent-context.json.collision.tmp";
        let temp_path = out_dir.join(temp_filename);

        // The first `create_new` open succeeds and leaves the temp file in place.
        let _first = open_agent_context_temp_file(&dir, temp_filename, &temp_path)
            .expect("first temp file creation should succeed");

        // A second open with the same name must fail hard: `create_new(true)`
        // refuses to clobber an existing temp file, preserving atomicity even
        // when a stale file lingers from a crashed run with a reused PID.
        let second = open_agent_context_temp_file(&dir, temp_filename, &temp_path);
        assert!(
            matches!(
                &second,
                Err(OrthohelpError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::AlreadyExists
            ),
            "expected create_new collision to report AlreadyExists, got {second:?}"
        );
    }

    #[test]
    fn temp_file_open_fails_when_file_already_exists() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let out_dir = Utf8Path::from_path(temp_dir.path()).expect("path is UTF-8");

        // `Dir` and `ambient_authority` are in scope via `use super::*`.
        // `open_ambient_dir` requires `AsRef<Utf8Path>`, so the `Utf8Path`
        // `out_dir` is passed directly rather than via `as_std_path()`, which
        // would yield a `std::path::Path` and fail the bound.
        let dir = Dir::open_ambient_dir(out_dir, ambient_authority()).expect("open temp dir");

        std::fs::File::create(out_dir.join("collision.tmp")).expect("pre-create collision file");

        let result =
            open_agent_context_temp_file(&dir, "collision.tmp", &out_dir.join("collision.tmp"));
        assert!(
            matches!(
                &result,
                Err(OrthohelpError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::AlreadyExists
            ),
            "expected create_new collision to report AlreadyExists, got {result:?}"
        );
    }

    #[test]
    fn concurrent_writes_produce_unique_temp_names() {
        let temp_dir = TempDir::new().expect("create temporary output directory");
        let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");

        let temp_filenames = (0..8)
            .map(|_| AgentContextWriteTarget::new(out_dir).temp_filename)
            .collect::<Vec<_>>();
        let unique_temp_filenames = temp_filenames.iter().collect::<HashSet<_>>();

        assert_eq!(unique_temp_filenames.len(), temp_filenames.len());
    }
}
