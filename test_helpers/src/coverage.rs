//! Helpers for forwarding child-process coverage configuration.
//!
//! Tests that launch a child process usually clear its environment so the
//! child sees only what the case supplies. That also drops `LLVM_PROFILE_FILE`,
//! and an instrumented child then writes its profile to a default location.
//! When the default location sits inside a fixture directory the test deletes,
//! the profile is lost; when it sits in the working tree, the profile is
//! written somewhere no collection step expects.
//!
//! Absolute profile paths already name their destination. Relative ones are
//! resolved against the parent's working directory because the child's
//! working directory is often the fixture directory that will be removed.
//!
//! # Examples
//!
//! ```
//! use std::ffi::OsStr;
//! use std::path::{Path, PathBuf};
//!
//! use ortho_config_test_helpers::coverage::resolve_profile_path;
//!
//! let parent = Path::new("/work");
//! assert_eq!(
//!     resolve_profile_path(OsStr::new("profiles/run.profraw"), parent),
//!     PathBuf::from("/work/profiles/run.profraw")
//! );
//! assert_eq!(
//!     resolve_profile_path(OsStr::new("/tmp/run.profraw"), parent),
//!     PathBuf::from("/tmp/run.profraw")
//! );
//! ```

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Resolve a coverage profile path for a child process.
///
/// Absolute paths are returned unchanged; relative paths are joined to
/// `parent_dir`, the directory the parent test ran in.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use std::path::{Path, PathBuf};
///
/// use ortho_config_test_helpers::coverage::resolve_profile_path;
///
/// assert_eq!(
///     resolve_profile_path(OsStr::new("run.profraw"), Path::new("/work")),
///     PathBuf::from("/work/run.profraw")
/// );
/// ```
#[must_use]
pub fn resolve_profile_path(profile_file: &OsStr, parent_dir: &Path) -> PathBuf {
    let profile_path = Path::new(profile_file);
    if profile_path.is_absolute() {
        return profile_path.to_path_buf();
    }
    parent_dir.join(profile_path)
}

#[cfg(test)]
mod tests {
    //! Unit tests for coverage path forwarding.

    use super::resolve_profile_path;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    #[test]
    fn relative_profile_paths_are_anchored_to_the_parent() {
        let parent = Path::new("/work");
        assert_eq!(
            resolve_profile_path(OsStr::new("profiles/run.profraw"), parent),
            PathBuf::from("/work/profiles/run.profraw")
        );
    }

    #[test]
    fn absolute_profile_paths_pass_through_unchanged() {
        let parent = Path::new("/work");
        assert_eq!(
            resolve_profile_path(OsStr::new("/tmp/run.profraw"), parent),
            PathBuf::from("/tmp/run.profraw")
        );
    }
}
