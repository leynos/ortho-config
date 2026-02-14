//! Helpers for safely mutating the process working directory in tests.
//!
//! The current working directory is process-global state requiring serialized
//! access across all test files.  This module provides an RAII guard that
//! acquires a global mutex, captures the original directory, and restores it
//! on drop — following the same pattern as [`crate::env`].
//!
//! # Examples
//!
//! ```no_run
//! use test_helpers::cwd;
//!
//! let guard = cwd::set_dir("/tmp/test-dir").expect("set cwd");
//! // CWD is now `/tmp/test-dir`; it is restored when `guard` is dropped.
//! ```

use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use parking_lot::Mutex;
use std::sync::LazyLock;

static CWD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

/// RAII guard that restores the working directory on drop.
#[must_use = "dropping restores the prior working directory"]
pub struct CwdGuard {
    original: Utf8PathBuf,
    _lock: parking_lot::MutexGuard<'static, ()>,
}

impl CwdGuard {
    /// Explicitly restores the original working directory.
    ///
    /// Prefer calling this over relying on [`Drop`] when the caller can
    /// handle the error — for example at the end of a test that returns
    /// `Result`.
    ///
    /// # Errors
    ///
    /// Returns an error if `set_current_dir` fails.
    pub fn restore(&self) -> std::io::Result<()> {
        std::env::set_current_dir(&self.original)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        // Best-effort restoration.  Callers that need to observe failures
        // should call `restore()` explicitly before the guard is dropped.
        let _unused = std::env::set_current_dir(&self.original);
    }
}

/// Changes the working directory to `path` and returns a guard that restores
/// the original directory on drop.
///
/// The global `CWD_MUTEX` is held for the lifetime of the returned guard,
/// ensuring no other test can mutate the working directory concurrently.
///
/// The original directory is captured and validated as UTF-8 *before* the
/// working directory is changed, so a UTF-8 conversion failure never leaves
/// the process in a different directory.
///
/// # Errors
///
/// Returns an error if the current directory cannot be read, is not valid
/// UTF-8, or the target path cannot be set.
pub fn set_dir(path: impl AsRef<std::path::Path>) -> Result<CwdGuard> {
    let lock = CWD_MUTEX.lock();
    let old = std::env::current_dir().context("read current dir")?;
    let old_utf8 = Utf8PathBuf::from_path_buf(old)
        .map_err(|non_utf8| anyhow!("cwd is not valid UTF-8: {}", non_utf8.display()))?;
    std::env::set_current_dir(path.as_ref()).context("set current dir")?;
    Ok(CwdGuard {
        original: old_utf8,
        _lock: lock,
    })
}
