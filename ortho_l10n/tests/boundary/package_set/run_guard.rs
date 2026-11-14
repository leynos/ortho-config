//! Excludes overlapping boundary runs from one checkout.
//!
//! The packaging lease answers "is a packaging step running right now". It does
//! not answer "may the roots I was just promised be replaced while I read
//! them", because it is released before the consumer reads anything. A contract
//! that reuses an earlier contract's roots gets its `Reused` answer from the
//! record, which is proof about *this run* rather than a lock on the directory.
//! That answer stays true only while no packaging step may replace the roots
//! behind it, so the exclusion has to cover the consumer's reads, not just the
//! record check.
//!
//! This module supplies that exclusion: a second lock, [`RunGuard`], taken by
//! every process that reaches the target and held for the process's whole life.
//! Holding it per process is the smallest span that covers the reads, which
//! outlive the call that promised them, and it costs nothing when no second run
//! is present.
//!
//! It is deliberately *not* the packaging lease, and it could not be folded
//! into it. The lease is taken and released around one step; widening it to the
//! consumer's reads would mean downgrading it to a shared lock, and
//! `File::lock_shared` on a handle that already holds the exclusive lock is
//! documented as unspecified and platform dependent, "including the possibility
//! that it will deadlock". Windows has no downgrade at all.
//!
//! # Why a separate file lock
//!
//! Same reason as the lease: `cap-std` exposes no locking API, so this is the
//! standard library's `flock` (Unix) or share-mode open (Windows), and
//! `dylint.toml` exempts this module from the capability-based filesystem
//! policy on the same terms - the only file it touches is its own guard in the
//! boundary's private tree, and the only thing it protects is that checkout.

#[cfg(test)]
#[path = "run_guard_tests.rs"]
mod run_guard_tests;

use super::lease::LEASE_POLL_INTERVAL;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use std::fs::File;
use std::io;
use std::time::Instant;

/// Name of the run guard, inside the boundary root.
///
/// Beside the packaging target rather than in it: it guards the whole
/// checkout's boundary tree, and the target is only one thing under it.
const RUN_GUARD_FILE_NAME: &str = ".boundary-run";

/// Hold the boundary's run guard for the rest of the process's life.
///
/// Every process that materializes or reads the packaged set calls this once,
/// before it touches the target, and keeps the returned guard alive until it
/// exits. That is what makes [`super::lease::Prepared::Reused`]'s answer safe:
/// a second run cannot begin replacing the roots while this one is still
/// reading them, because it cannot get past this lock to do it.
///
/// The wait is bounded by `deadline`, the calling contract's shared allowance,
/// so a second run reports the contention rather than hanging behind it.
/// Callers that hold no allowance (the regressions) pass [`Instant::now`].
///
/// # Errors
///
/// Returns an error when the guard cannot be created or locked, or when another
/// run held it past `deadline`.
pub(crate) fn hold_run(boundary: &Utf8Path, deadline: Instant) -> Result<RunGuard> {
    let directory = open_directory(boundary)?;
    let guard_path = boundary.join(RUN_GUARD_FILE_NAME);
    loop {
        match try_guard(&directory, &guard_path) {
            Ok(guard) => return Ok(guard),
            Err(error) if is_held_elsewhere(&error) => {}
            Err(error) => return Err(error).context("lock the boundary run guard"),
        }
        let left = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !left.is_zero(),
            "another boundary run holds {guard_path} for this checkout; \
             overlapping runs would replace the packaged roots one another is \
             reading, so this contract stopped instead",
        );
        std::thread::sleep(LEASE_POLL_INTERVAL.min(left));
    }
}

/// The boundary's run guard, held for a process's whole life.
///
/// Dropping it releases the checkout to another run, which is why callers keep
/// it in a `OnceLock` rather than a local.
pub(crate) struct RunGuard {
    /// The locked file. Never read; kept so the kernel lock outlives the call.
    _file: File,
}

/// Open the guard file and take its exclusive lock without waiting.
///
/// Unix takes it through a capability handle and relies on `flock`, so the open
/// itself never waits.
#[cfg(not(windows))]
fn try_guard(directory: &Dir, _path: &Utf8Path) -> io::Result<RunGuard> {
    use std::fs::TryLockError;

    let file = directory
        .create(RUN_GUARD_FILE_NAME)
        .map(cap_std::fs_utf8::File::into_std)?;
    match file.try_lock() {
        Ok(()) => Ok(RunGuard { _file: file }),
        Err(TryLockError::WouldBlock) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Open the guard file and take its exclusive lock without waiting.
///
/// Windows reuses the lease's deny-share open, because the guard wants exactly
/// the same exclusivity; the two differ only in what they protect. Takes the
/// path rather than the handle because the deny-share open this uses is a `std`
/// one, exactly as the lease's writer side is.
#[cfg(windows)]
fn try_guard(_directory: &Dir, path: &Utf8Path) -> io::Result<RunGuard> {
    use std::os::windows::fs::OpenOptionsExt;

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .share_mode(0)
        .open(path.as_std_path())?;
    Ok(RunGuard { _file: file })
}

/// Open `path`, creating it only if it is not there yet.
///
/// The boundary root may not exist on a clean checkout, and creating it is the
/// ordinary first step rather than a race to report.
///
/// # Errors
///
/// Returns an error when the path exists but cannot be opened, or when it is
/// absent and cannot be created.
fn open_directory(path: &Utf8Path) -> Result<Dir> {
    match Dir::open_ambient_dir(path, cap_std::ambient_authority()) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Dir::create_ambient_dir_all(path, cap_std::ambient_authority())
                .with_context(|| format!("create {path}"))?;
            Dir::open_ambient_dir(path, cap_std::ambient_authority())
                .with_context(|| format!("open {path}"))
        }
        Err(error) => Err(error).with_context(|| format!("open {path}")),
    }
}

/// Whether an open or lock failed because another run holds the guard.
///
/// Windows reports a refused exclusive open as `ERROR_SHARING_VIOLATION`; Unix
/// reports a held `flock` as `WouldBlock`. Both mean "try again later".
fn is_held_elsewhere(error: &io::Error) -> bool {
    #[cfg(windows)]
    {
        /// `ERROR_SHARING_VIOLATION` from `Win32::Foundation`.
        const ERROR_SHARING_VIOLATION: i32 = 32;
        error.raw_os_error() == Some(ERROR_SHARING_VIOLATION)
    }
    #[cfg(not(windows))]
    {
        error.kind() == io::ErrorKind::WouldBlock
    }
}
