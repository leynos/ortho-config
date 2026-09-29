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

/// Records a path-specific `InvalidInput` failure for malformed file requests.
pub(super) fn invalid_input(path: &Path, msg: impl Into<String>) -> Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::InvalidInput, msg.into()),
    )
}

/// Records a path-specific `InvalidData` failure when file contents violate
/// the selected format's contract.
pub(super) fn invalid_data(path: &Path, msg: impl Into<String>) -> Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::InvalidData, msg.into()),
    )
}

/// Records a path-specific `NotFound` failure when a required referenced file
/// is absent.
pub(super) fn not_found(path: &Path, msg: impl Into<String>) -> Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::NotFound, msg.into()),
    )
}
