//! Helpers for safely mutating the process working directory in tests.
//!
//! The current working directory is process-global state requiring serialised
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

impl Drop for CwdGuard {
    fn drop(&mut self) {
        // PANIC: Drop cannot return Result; failing to restore the cwd would
        // leave the test environment in a broken state, so we panic to fail
        // fast.
        if let Err(err) = std::env::set_current_dir(&self.original) {
            panic!("restore current dir: {err}");
        }
    }
}

/// Changes the working directory to `path` and returns a guard that restores
/// the original directory on drop.
///
/// The global `CWD_MUTEX` is held for the lifetime of the returned guard,
/// ensuring no other test can mutate the working directory concurrently.
///
/// # Errors
///
/// Returns an error if the current directory cannot be read, is not valid
/// UTF-8, or the target path cannot be set.
pub fn set_dir(path: impl AsRef<std::path::Path>) -> Result<CwdGuard> {
    let lock = CWD_MUTEX.lock();
    let old = std::env::current_dir().context("read current dir")?;
    std::env::set_current_dir(path.as_ref()).context("set current dir")?;
    let old_utf8 = Utf8PathBuf::from_path_buf(old)
        .map_err(|non_utf8| anyhow!("cwd is not valid UTF-8: {}", non_utf8.display()))?;
    Ok(CwdGuard {
        original: old_utf8,
        _lock: lock,
    })
}
