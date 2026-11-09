//! Cross-process lease over the identifier-artefact fixture's private target.
//!
//! A fixed target directory is shared state: a second `cargo test`, `make test`
//! or nextest invocation for another target can reach the same fixture and run
//! its own `cargo clean -p orthohelp_fixture` inside this process's build or
//! artefact read. nextest's process group and `#[serial]` only coordinate one
//! runner, so the exclusion has to come from the operating system.
//!
//! `cap-std` exposes no file-locking API, so this module is the one place in
//! the identifier-artefact target that uses the standard library's lock. It is
//! accordingly exempted, by a single module-scoped entry in `dylint.toml`, from
//! the capability policy; every other filesystem access in the target keeps
//! using `cap_std` handles. The file it opens is created inside the fixture's
//! own private target, never an ambient path chosen by this module.

use anyhow::{Context, Result, ensure};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::fs::{File, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

/// Names the lease file that serializes whole tests over the private target.
///
/// The file lives inside the target so it shares the target's lifetime and its
/// `CACHEDIR.TAG` marking. Its contents are irrelevant: the lease is the
/// kernel-held lock on it, released when the owning handle closes.
const LEASE_FILE_NAME: &str = ".identifier-artefact.lease";

/// How often a blocked leaseholder re-checks the target.
///
/// The wait is measured in hundreds of seconds, so polling this often costs
/// nothing measurable while keeping the hand-off to the next holder prompt.
const LEASE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// A lease held for as long as the test owns the fixture.
///
/// The lock lives exactly as long as this value: dropping it releases the
/// lease, which is why the fixture carries it for the whole test.
pub(super) struct TargetLease {
    /// The locked file. Never read; kept so the kernel lock outlives the call.
    _file: File,
}

/// Open, creating if needed, and exclusively lock the target's lease file.
///
/// Waits until every other leaseholder has released the target, so a
/// concurrent test process cannot interleave its `cargo clean -p` with this
/// test's build or artefact read. The wait is bounded by `deadline`, which the
/// caller takes from the scenario envelope, so a target held by another runner
/// produces this diagnostic rather than nextest terminating the test with
/// nothing to say which wait it was in.
///
/// # Errors
///
/// Returns an error when the target cannot be opened or locked, or when
/// `deadline` passes while another process still holds the lease.
pub(super) fn acquire(target: &Path, deadline: Instant) -> Result<TargetLease> {
    let directory = Dir::open_ambient_dir(target, ambient_authority())
        .context("open the identifier-artefact target for its lease")?;
    // `truncate(false)` matters: the lease is the lock on the file, and
    // clearing it would be a spurious write racing with another opener.
    let file = directory
        .open_with(
            LEASE_FILE_NAME,
            OpenOptions::new().create(true).write(true).truncate(false),
        )
        .context("open the identifier-artefact target lease")?
        .into_std();
    acquire_within(file, target, deadline)
}

/// Take the lease on `file`, giving up at `deadline`.
///
/// `file` is moved into the returned [`TargetLease`] on success, so the kernel
/// lock is released exactly when the test finishes rather than when this
/// function's local binding goes out of scope.
fn acquire_within(file: File, target: &Path, deadline: Instant) -> Result<TargetLease> {
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(TargetLease { _file: file }),
            Err(TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(
                    !remaining.is_zero(),
                    "another test process held the identifier-artefact lease {} for the \
                     whole scenario envelope; its `cargo clean -p orthohelp_fixture` could \
                     remove this build's artefact, so this test stopped instead of building \
                     beside it",
                    target.join(LEASE_FILE_NAME).display(),
                );
                std::thread::sleep(LEASE_POLL_INTERVAL.min(remaining));
            }
            Err(TryLockError::Error(error)) => {
                return Err(error).context("lease the identifier-artefact target");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! The bounded wait itself, without a second process.

    use super::{LEASE_FILE_NAME, acquire};
    use anyhow::{Context, Result, ensure};
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    /// A lease held past the deadline is abandoned, naming the lease file.
    ///
    /// The operating system denies an exclusive lock to a second handle on the
    /// same file even inside one process, so the first `TargetLease` is enough
    /// to make the second call block. Without the bound this test would hang
    /// until the harness stopped it, which is the failure the deadline exists
    /// to replace with a diagnostic.
    #[test]
    fn an_exhausted_wait_names_the_lease_file() -> Result<()> {
        let target = TempDir::new().context("create the lease target")?;
        let _held = acquire(target.path(), Instant::now() + Duration::from_secs(5))?;
        let error = acquire(target.path(), Instant::now() + Duration::from_millis(200))
            .err()
            .context("a second handle acquired a lease this process already holds")?;
        ensure!(
            format!("{error:#}").contains(LEASE_FILE_NAME),
            "the exhausted wait should name {LEASE_FILE_NAME:?}:\n{error:#}",
        );
        Ok(())
    }
}
