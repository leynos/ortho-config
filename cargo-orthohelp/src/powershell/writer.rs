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

/// Request describing a text file write operation.
pub struct TextWriteRequest<'a> {
    /// Root directory for error reporting.
    pub root: &'a Utf8Path,
    /// Target path relative to the root directory.
    pub relative_path: &'a Utf8Path,
    /// Text content to write.
    pub content: &'a str,
    /// Whether to prepend a UTF-8 BOM.
    pub include_bom: bool,
}

/// Writes text content to a file with CRLF line endings.
pub fn write_crlf_text(
    dir: &Dir,
    request: &TextWriteRequest<'_>,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let mut file = dir
        .open_with(
            request.relative_path,
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: request.root.join(request.relative_path),
            source: io_err,
        })?;

    if request.include_bom {
        file.write_all(&UTF8_BOM)
            .map_err(|io_err| OrthohelpError::Io {
                path: request.root.join(request.relative_path),
                source: io_err,
            })?;
    }

    file.write_all(request.content.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: request.root.join(request.relative_path),
            source: io_err,
        })?;

    Ok(request.root.join(request.relative_path))
}
