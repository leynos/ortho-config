//! The advisory lock that serialises the clap error fixture build.
//!
//! nextest runs every test in its own process, so several processes of one run
//! can reach the fixture build at once. Serialising them needs one meeting
//! point those processes already share: an exclusive operating-system lock on
//! an empty file beside the test binary.
//!
//! `fs4` locks a `std::fs::File` and nothing else, and `cap-std` exposes no
//! locking API, so this module is the suite's one step outside the
//! capability-based filesystem policy. It creates that empty file and touches
//! nothing else; `dylint.toml` records the exemption under
//! `no_std_fs_operations`.

use super::build_lock_path;
use anyhow::{Context, Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use fs4::{FileExt, TryLockError};
use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

/// Holds the advisory lock that serialises fixture builds across processes.
///
/// The returned handle releases the lock when it is dropped, which happens
/// before this function's caller runs the fixture, so a second process may
/// build while the first is already executing its cases.
///
/// The wait is bounded by `budget` measured from `started`, so a caller that
/// shares one budget between this wait and a build keeps the total inside it.
/// The budget is carried separately from the deadline because the failure
/// message must name the budget the caller set: a caller waiting 200
/// milliseconds must not be told that a 600-second budget elapsed.
pub(super) fn acquire(started: Instant, budget: Duration) -> Result<File> {
    let path = build_lock_path()?;
    let lock = open_lock_file(&path)?;
    wait_until_locked(&lock, &path, started + budget, budget)?;
    Ok(lock)
}

/// Opens the empty lock file, creating it on first use.
///
/// The parent directory is opened as a capability, and the file is created
/// through that capability, so nothing outside it can be reached by mistake.
fn open_lock_file(path: &Path) -> Result<File> {
    let parent = path
        .parent()
        .with_context(|| format!("{} should sit in a directory", path.display()))?;
    let name = path
        .file_name()
        .with_context(|| format!("{} should name a file", path.display()))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .with_context(|| format!("open {} to serialise the fixture build", parent.display()))?;
    let lock = directory
        .open_with(name, OpenOptions::new().read(true).write(true).create(true))
        .with_context(|| format!("open {}", path.display()))?
        .into_std();
    Ok(lock)
}

/// Polls until the lock is held, or reports that the wait outlived `deadline`.
///
/// Polling rather than blocking keeps the wait inside one budget shared with
/// the build, so a lock held by a stuck process is reported instead of
/// consuming the test's whole nextest allowance. `budget` is the interval the
/// caller started counting from; it is reported verbatim so the message
/// describes the wait that actually happened.
fn wait_until_locked(lock: &File, path: &Path, deadline: Instant, budget: Duration) -> Result<()> {
    loop {
        match FileExt::try_lock(lock) {
            Ok(()) => return Ok(()),
            Err(TryLockError::Error(error)) => {
                return Err(error).context("lock the clap error fixture build");
            }
            Err(TryLockError::WouldBlock) => {
                ensure!(
                    Instant::now() < deadline,
                    "the clap error fixture build lock stayed held past its {budget:?} budget: {}",
                    path.display(),
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
