//! Error constructors shared by file loading helpers.

use crate::OrthoError;

use std::error::Error;
use std::path::Path;
use std::sync::Arc;

/// Construct an [`OrthoError::File`] for a configuration path.
pub(super) fn file_error(
    path: &Path,
    err: impl Into<Box<dyn Error + Send + Sync>>,
) -> Arc<OrthoError> {
    Arc::new(OrthoError::File {
        path: path.to_path_buf(),
        source: err.into(),
    })
}

pub(super) fn invalid_input(path: &Path, msg: impl Into<String>) -> Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::InvalidInput, msg.into()),
    )
}

pub(super) fn invalid_data(path: &Path, msg: impl Into<String>) -> Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::InvalidData, msg.into()),
    )
}

pub(super) fn not_found(path: &Path, msg: impl Into<String>) -> Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::NotFound, msg.into()),
    )
}
