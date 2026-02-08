//! File writing helpers for `PowerShell` output.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::io::Write;

use crate::error::OrthohelpError;

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Ensures a directory exists and returns a handle to it.
pub fn ensure_dir(path: &Utf8Path) -> Result<Dir, OrthohelpError> {
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

/// Writes text content to a file with CRLF line endings.
pub struct WriteTarget<'a> {
    /// Root directory for error reporting.
    pub root: &'a Utf8Path,
    /// Target path relative to the root directory.
    pub relative_path: &'a Utf8Path,
}

/// Writes text content to a file with CRLF line endings.
pub fn write_crlf_text(
    dir: &Dir,
    target: &WriteTarget<'_>,
    content: &str,
    include_bom: bool,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let full_path = target.root.join(target.relative_path);
    let mut file = dir
        .open_with(
            target.relative_path,
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: full_path.clone(),
            source: io_err,
        })?;

    if include_bom {
        file.write_all(&UTF8_BOM)
            .map_err(|io_err| OrthohelpError::Io {
                path: full_path.clone(),
                source: io_err,
            })?;
    }

    file.write_all(content.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: full_path.clone(),
            source: io_err,
        })?;

    Ok(full_path)
}
