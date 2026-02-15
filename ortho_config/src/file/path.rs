//! Filesystem path helpers used while resolving `extends` relationships.

use crate::{OrthoError, OrthoResult};

use std::path::{Path, PathBuf};

use super::error::{file_error, invalid_input, not_found};

/// Canonicalise `p` using platform-specific rules.
///
/// Returns an absolute, normalized path with symlinks resolved.
///
/// On Windows the [`dunce`](https://docs.rs/dunce/latest/dunce/) crate is used to avoid introducing UNC prefixes
/// in diagnostic messages.
///
/// # Errors
///
/// Returns an [`OrthoError`] if canonicalization fails.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::Path;
///
/// # fn run() -> ortho_config::OrthoResult<()> {
/// let p = Path::new("config.toml");
/// let c = ortho_config::file::canonicalise(p)?;
/// assert!(c.is_absolute());
/// # Ok(())
/// # }
/// ```
pub fn canonicalise(p: &Path) -> OrthoResult<PathBuf> {
    #[cfg(windows)]
    {
        dunce::canonicalize(p).map_err(|e| file_error(p, e))
    }
    #[cfg(not(windows))]
    {
        std::fs::canonicalize(p).map_err(|e| file_error(p, e))
    }
}

/// Normalize a canonical path for case-insensitive cycle detection.
///
/// The loader stores normalized keys in its visited set to ensure that files
/// referenced with different casing are treated as the same node when the
/// filesystem ignores case differences. On strictly case-sensitive platforms
/// the path is returned unchanged.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::Path;
///
/// let canonical = Path::new("/configs/Config.toml");
/// let key = ortho_config::file::normalize_cycle_key(canonical);
/// // On Windows and macOS the key is lower-cased so variants like
/// // "/configs/config.toml" do not bypass cycle detection.
/// ```
pub(super) fn normalize_cycle_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        // Windows performs ASCII-only case folding when comparing paths. Apply the
        // same transformation so canonicalized cycle keys match the filesystem's
        // semantics without mutating non-ASCII characters.
        let lowered: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .map(|unit| {
                if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
                    unit + 32
                } else {
                    unit
                }
            })
            .collect();
        PathBuf::from(OsString::from_wide(&lowered))
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::OsString;

        let lowered = match path.as_os_str().to_str() {
            Some(text) => text.to_lowercase(),
            None => return path.to_path_buf(),
        };
        PathBuf::from(OsString::from(lowered))
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        path.to_path_buf()
    }
}

/// Resolve an `extends` path relative to the current file.
///
/// If `base` is relative it is joined with the parent directory of
/// `current_path` and canonicalized. Absolute paths are canonicalized
/// directly.
///
/// Canonicalization ensures consistent absolute paths for robust cycle
/// detection and de-duplication across symlinks. On Windows this uses
/// [`dunce::canonicalize`] to avoid introducing UNC prefixes in diagnostic
/// messages.
///
/// The target must already exist as a regular file. If the file is missing, a
/// not-found error is returned describing the absolute path derived from
/// `current_path`.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the parent directory cannot be determined
/// or if canonicalization fails.
///
/// # Examples
///
/// ```rust,ignore
/// # use std::path::{Path, PathBuf};
/// # use ortho_config::file::resolve_base_path;
/// # fn run() -> ortho_config::OrthoResult<()> {
/// let current = Path::new("/tmp/config.toml");
/// let base = PathBuf::from("base.toml");
/// let canonical = resolve_base_path(current, base)?;
/// assert!(canonical.ends_with("base.toml"));
/// # Ok(())
/// # }
/// ```
pub(super) fn resolve_base_path(current_path: &Path, base: PathBuf) -> OrthoResult<PathBuf> {
    let parent = current_path.parent().ok_or_else(|| {
        invalid_input(
            current_path,
            "Cannot determine parent directory for config file when resolving 'extends'",
        )
    })?;
    let resolved_base = if base.is_absolute() {
        base
    } else {
        canonicalise(parent)?.join(base)
    };
    match canonicalise(&resolved_base) {
        Ok(path) => Ok(path),
        Err(err) => {
            let OrthoError::File { source, .. } = err.as_ref() else {
                return Err(err);
            };
            let Some(io_err) = source.downcast_ref::<std::io::Error>() else {
                return Err(err);
            };
            if io_err.kind() != std::io::ErrorKind::NotFound {
                return Err(err);
            }
            Err(not_found(
                &resolved_base,
                format!(
                    "extended configuration file '{}' does not exist (referenced from '{}')",
                    resolved_base.display(),
                    current_path.display()
                ),
            ))
        }
    }
}
