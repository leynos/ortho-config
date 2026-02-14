//! Shared filesystem helpers for configuration file loading.

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Return the parent directory of `path`, falling back to `"."` when the path
/// has no parent or the parent is empty.
pub(super) fn parent_or_dot(path: &Utf8Path) -> &Utf8Path {
    path.parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or_else(|| Utf8Path::new("."))
}

/// Open the parent directory of `path` via `cap-std` and extract the file name.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if the file name cannot be determined or the
/// parent directory cannot be opened.
pub(super) fn open_parent_dir_and_name(path: &Utf8Path) -> std::io::Result<(Dir, String)> {
    let parent = parent_or_dot(path);
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::other("cannot determine file name for configuration file path")
    })?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok((dir, file_name.to_owned()))
}
