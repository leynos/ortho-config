//! Filesystem helpers shared across `cargo-orthohelp` modules.

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use crate::error::OrthohelpError;

/// Opens a directory if it exists, returning `None` when the path is missing.
pub fn open_optional_dir(path: &Utf8Path) -> Result<Option<Dir>, OrthohelpError> {
    match Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(dir) => Ok(Some(dir)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(OrthohelpError::Io {
            path: path.to_path_buf(),
            source: err,
        }),
    }
}
