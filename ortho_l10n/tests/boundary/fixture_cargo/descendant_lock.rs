//! Observing a descendant process through an operating-system file lock.
//!
//! A probe cannot see its own descendant, so the descendant takes an exclusive
//! lock on a file in the scratch directory it shares with its parent, and the
//! parent asks the kernel whether that lock is still held. The kernel drops the
//! lock when the process dies, however it dies, so the answer can never go
//! stale while a process is wedged — unlike a heartbeat or a marker file, which
//! a hung process stops updating but does not remove.
//!
//! `cap-std` exposes no locking API, so the only portable lock available is the
//! standard library's. This module is therefore deliberately exempted from the
//! capability-based filesystem policy in `dylint.toml`. The only file it
//! touches is the probe's own lock file inside the scratch directory its parent
//! supplied; no other ambient path is reachable from here.

use super::{DESCENDANT_TEARDOWN, READY_FILE_NAME, scratch_directory, wait_until};
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use std::fs::{File, TryLockError};
use std::path::Path;

/// Name of the lock file a descendant holds for as long as it lives.
pub(crate) const LOCK_FILE_NAME: &str = "descendant.lock";

/// A held descendant lock.
///
/// The lock lives exactly as long as this value: dropping it releases the
/// lock, and so does the death of the process that holds it.
pub(crate) struct HeldLock {
    /// The locked file. Kept only so that the lock outlives this call.
    _file: File,
}

/// Take the descendant's exclusive lock, then announce readiness.
///
/// The lock outlives the descendant's sleep because only process death releases
/// it. That is what lets the parent distinguish a live descendant from a killed
/// one without polling for heartbeats.
pub(crate) fn hold_descendant_lock() -> Result<HeldLock> {
    let scratch = scratch_directory()?;
    let file = scratch
        .create(LOCK_FILE_NAME)
        .context("create the descendant lock file")?
        .into_std();
    file.lock().context("take the descendant lock")?;
    scratch
        .write(READY_FILE_NAME, b"locked")
        .context("announce the descendant lock")?;
    Ok(HeldLock { _file: file })
}

/// Wait for the descendant's lock to be released, proving it is gone.
pub(crate) fn wait_for_lock_release(scratch: &Path) -> Result<()> {
    let file = open_lock_file(scratch)?;
    wait_until(
        DESCENDANT_TEARDOWN,
        "the killed descendant to release its lock",
        || match file.try_lock() {
            Ok(()) => Ok(true),
            Err(TryLockError::WouldBlock) => Ok(false),
            Err(TryLockError::Error(error)) => {
                Err(error).context("try to lock the descendant lock file")
            }
        },
    )
}

/// Report whether a live process still holds the descendant's lock.
///
/// A `true` result also proves the caller took and released the lock itself, so
/// it cannot be used while another check depends on the lock being held.
pub(crate) fn lock_is_held(scratch: &Path) -> Result<bool> {
    let file = open_lock_file(scratch)?;
    match file.try_lock() {
        Ok(()) => Ok(false),
        Err(TryLockError::WouldBlock) => Ok(true),
        Err(TryLockError::Error(error)) => {
            Err(error).context("try to lock the descendant lock file")
        }
    }
}

/// Open the descendant's lock file for a lock probe.
fn open_lock_file(scratch: &Path) -> Result<File> {
    Ok(Dir::open_ambient_dir(scratch, ambient_authority())
        .context("open the probe scratch directory")?
        .open(LOCK_FILE_NAME)
        .context("open the descendant lock file")?
        .into_std())
}
