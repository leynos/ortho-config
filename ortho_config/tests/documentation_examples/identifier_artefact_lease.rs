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
//!
//! The lease is more than a mutex between test processes. Every nested Cargo
//! invocation inherits a duplicate of the holder's handle as its standard
//! input, so the operating system keeps the lease held for as long as any
//! descendant of that invocation is still able to write to the target. A
//! replacement process therefore cannot acquire the target between a helper's
//! death and the death of the compilers beneath it. On Unix the duplicate
//! shares the open file description that `flock` locks. On Windows, where
//! `LockFileEx` locks belong to the locking process rather than to the file
//! object, the lease file is opened with an empty share mode, so an inherited
//! duplicate keeps a later open refused.

use anyhow::{Context, Result, ensure};
#[cfg(not(windows))]
use cap_std::fs::OpenOptions;
use cap_std::{ambient_authority, fs::Dir};
use std::fs::{File, TryLockError};
use std::io;
use std::path::Path;
use std::process::Stdio;
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
/// lease, unless a nested child still holds a duplicate obtained from
/// [`TargetLease::inherited_handle`]. The fixture therefore carries it for the
/// whole test.
pub(crate) struct TargetLease {
    /// The locked file. Never read; kept so the kernel lock outlives the call.
    file: File,
}

impl TargetLease {
    /// A duplicate of the lease handle for a nested child to inherit.
    ///
    /// Passing this as the child's standard input hands every descendant of
    /// that child the same open file description (Unix) or file object
    /// (Windows). The lease then remains held until the last of those
    /// processes exits, which is what stops a replacement leaseholder from
    /// cleaning the target while an orphaned compiler can still write to it.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot duplicate the handle.
    pub(crate) fn inherited_handle(&self) -> Result<Stdio> {
        let duplicate = self
            .file
            .try_clone()
            .context("duplicate the identifier-artefact lease handle")?;
        Ok(Stdio::from(duplicate))
    }
}

/// Create the target if needed and exclusively lock its lease file.
///
/// Waits until every other leaseholder has released the target, so a
/// concurrent test process cannot interleave its `cargo clean -p` with this
/// test's build or artefact read. The wait is bounded by `deadline`, which the
/// caller takes from the scenario envelope, so a target held by another runner
/// produces this diagnostic rather than nextest terminating the test with
/// nothing to say which wait it was in.
///
/// The directory is created here rather than during discovery, so the first
/// write to the target happens only once the lease is the only writer.
///
/// # Errors
///
/// Returns an error when the target cannot be opened or locked, or when
/// `deadline` passes while another process still holds the lease.
pub(crate) fn acquire(target: &Path, deadline: Instant) -> Result<TargetLease> {
    Dir::create_ambient_dir_all(target, ambient_authority())
        .context("create the identifier-artefact target for its lease")?;
    let directory = Dir::open_ambient_dir(target, ambient_authority())
        .context("open the identifier-artefact target for its lease")?;
    let path = target.join(LEASE_FILE_NAME);
    acquire_within(&directory, &path, target, deadline)
}

/// Take the lease on the file at `path`, giving up at `deadline`.
///
/// The open is inside the wait, not before it: on Windows the lease is an
/// exclusive open, so a holder, or a descendant that inherited its handle,
/// refuses the open itself rather than only the lock. Retrying the whole
/// open-and-lock step also means the diagnostic below covers both ways a
/// holder can keep this process out.
///
/// The successful [`File`] is moved into the returned [`TargetLease`], so the
/// kernel lock is released exactly when the test finishes rather than when
/// this function's local binding goes out of scope.
fn acquire_within(
    directory: &Dir,
    path: &Path,
    target: &Path,
    deadline: Instant,
) -> Result<TargetLease> {
    loop {
        match open_lease(directory, path) {
            Ok(file) => match file.try_lock() {
                Ok(()) => return Ok(TargetLease { file }),
                Err(TryLockError::WouldBlock) => {}
                Err(TryLockError::Error(error)) => {
                    return Err(error).context("lease the identifier-artefact target");
                }
            },
            Err(error) if is_held_elsewhere(&error) => {}
            Err(error) => {
                return Err(error).context("open the identifier-artefact target lease");
            }
        }
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
}

/// Open the lease file without taking its lock.
///
/// Unix opens it through the target's capability handle and relies on `flock`.
/// Windows has to deny sharing at open time: an inherited duplicate of the
/// holder's handle then keeps every later open refused for as long as the
/// descendant holding it lives, which `LockFileEx` alone cannot do because its
/// locks belong to the locking process.
#[cfg(not(windows))]
fn open_lease(directory: &Dir, _path: &Path) -> io::Result<File> {
    // `truncate(false)` matters: the lease is the lock on the file, and
    // clearing it would be a spurious write racing with another opener.
    directory
        .open_with(
            LEASE_FILE_NAME,
            OpenOptions::new().create(true).write(true).truncate(false),
        )
        .map(cap_std::fs::File::into_std)
}

/// Open the lease file with no sharing permitted.
#[cfg(windows)]
fn open_lease(_directory: &Dir, path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    // `share_mode(0)` denies read, write and delete to every other open,
    // including one from this process, and an inherited duplicate keeps the
    // file object open after the holder exits.
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .share_mode(0)
        .open(path)
}

/// Whether an open failed because another handle forbids sharing.
///
/// Windows reports a refused exclusive open as `ERROR_SHARING_VIOLATION`.
/// Unix never refuses the open; the lock refuses the lease instead.
#[cfg(windows)]
fn is_held_elsewhere(error: &io::Error) -> bool {
    /// `ERROR_SHARING_VIOLATION` from `Win32::Foundation`.
    const ERROR_SHARING_VIOLATION: i32 = 32;
    error.raw_os_error() == Some(ERROR_SHARING_VIOLATION)
}

/// Whether an open failed because another handle forbids sharing.
#[cfg(not(windows))]
const fn is_held_elsewhere(_error: &io::Error) -> bool {
    false
}

#[cfg(test)]
mod tests {
    //! The bounded wait itself, without a second process.

    use super::{LEASE_FILE_NAME, acquire};
    use anyhow::{Context, Result, ensure};
    use std::sync::mpsc;
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

    /// A wait ends at a deadline that an earlier phase already partly spent.
    ///
    /// `acquire` takes the deadline as a parameter, so this cannot observe a
    /// second envelope being started inside it; that regression is caught by
    /// the Python contract `test_the_identifier_fixture_charges_discovery_to_one_envelope`,
    /// which inspects the implementation. What this test can show is the
    /// property the parameter exists for: an envelope already 400 ms spent
    /// against a 600 ms deadline ends the wait at the deadline, not at a full
    /// fresh one. The finishing instant is reported back from the worker so
    /// the assertion measures when the wait ended rather than merely that it
    /// ended before the receive timeout.
    #[test]
    fn a_wait_ends_at_a_deadline_already_partly_spent() -> Result<()> {
        let target = TempDir::new().context("create the lease target")?;
        let _held = acquire(target.path(), Instant::now() + Duration::from_secs(5))?;
        // Discovery's share of the envelope, spent before the wait begins.
        let started = Instant::now();
        let deadline = started + Duration::from_millis(600);
        std::thread::sleep(Duration::from_millis(400));
        let path = target.path().to_path_buf();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let report = match acquire(&path, deadline) {
                Ok(_) => "acquired a lease another handle still holds".to_owned(),
                Err(error) => format!("{error:#}"),
            };
            let _receiver_gone = sender.send((report, Instant::now()));
        });
        let (report, finished) = receiver
            .recv_timeout(Duration::from_secs(10))
            .context("the wait did not end when the shared envelope expired")?;
        ensure!(
            report.contains(LEASE_FILE_NAME),
            "the wait should end at the shared envelope and name {LEASE_FILE_NAME:?}:\n{report}",
        );
        // Generous against a loaded host, and still far below the 600 ms the
        // deadline allows: a wait that started its own envelope would sit on
        // the held lease for its full span rather than the 200 ms remainder.
        ensure!(
            finished.duration_since(started) < Duration::from_secs(2),
            "the wait outlasted the shared envelope by {:?}",
            finished.duration_since(started),
        );
        Ok(())
    }
}
