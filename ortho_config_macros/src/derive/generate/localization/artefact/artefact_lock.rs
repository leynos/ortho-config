//! Inter-process serialisation of identifier artefact publication.
//!
//! `OUT_DIR` is scoped to the package, not to the target being compiled, so a
//! package with several targets can have several compiler processes reaching
//! one artefact directory at once. Procedural-macro side effects carry no
//! ordering or serialisation guarantee, so without this lock two writers can
//! interleave and lose entries: a writer that merged before a peer's fragment
//! landed would replace the peer's complete inventory with that stale
//! rendering.
//!
//! A unique temporary filename is not a substitute. It removes the collision
//! on the shared `.tmp` path, but it leaves the read-modify-write of the
//! merged inventory unsynchronised. The lock therefore spans the whole
//! sequence: fragment write, merge, and rendered-file publication.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use proc_macro2::Span;

/// Names the lock file created inside the artefact directory.
const LOCK_FILE: &str = "cli-identifiers.lock";

/// Holds the exclusive artefact-publication lock for its whole lifetime.
///
/// Dropping the guard explicitly unlocks before the underlying file handle
/// closes, which covers the caller's error paths as well as its success path.
pub(super) struct LockGuard {
    /// Keeps the locked handle open; the guard owns the lock's lifetime.
    file: fs::File,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // A concurrent fork can briefly inherit this open-file description
        // before exec closes its close-on-exec descriptor. Explicit unlock
        // prevents that child from prolonging the completed critical section.
        // Drop cannot report an unlock error; closing the parent handle remains
        // the best-effort fallback.
        let _unlock_result = self.file.unlock();
    }
}

/// How long a reporting acquirer sleeps between attempts.
///
/// Only the reporting path polls. It must surface its own wait *while* it is
/// still waiting, and a blocking [`fs::File::lock`] cannot say anything until
/// the lock has already been granted — the difference between explaining a
/// hung build and describing a completed one. The interval is short enough
/// that a released lock is noticed promptly and long enough that a blocked
/// build does not spin.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Creates the lock file if absent and opens it without truncating.
///
/// `create` rather than `write`/`create_new` so a re-run does not clear a
/// lock file a peer is currently holding: the file carries no contents, only
/// the lock.
fn create_lock_file(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
}

/// Opens an existing lock file without creating or truncating it.
///
/// A read-only handle can take a shared or exclusive lock, so acquiring
/// through it mutates nothing. A caller that only reports the lock's state
/// therefore stays a pure query: it cannot bring a lock file into existence,
/// which an exclusive acquisition would then hold.
#[cfg(test)]
fn open_existing_lock_file(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().read(true).open(path)
}

/// Opens the lock file, mapping an open failure to a spanned diagnostic.
///
/// Opening is separated from acquiring because the acquisition paths differ
/// while the path resolution and the failure message they must preserve do not.
fn open_lock_file(path: &Path) -> syn::Result<fs::File> {
    create_lock_file(path).map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!(
                "cannot open identifier artefact lock {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                path.display()
            ),
        )
    })
}

/// Resolves the lock file's path inside an artefact directory.
///
/// Every acquirer and the test probe resolve through here, so [`LOCK_FILE`] is
/// named once. The acquisition paths differ in how they take the lock and what
/// they report, not in which file they contend for.
pub(super) fn lock_path(root: &Path) -> PathBuf {
    root.join(LOCK_FILE)
}

/// Builds the diagnostic for a lock that could not be acquired.
///
/// Both acquisition paths report the same failure, so the wording lives in one
/// place rather than being restated per path and drifting apart.
fn lock_error(root: &Path, error: &std::io::Error) -> syn::Error {
    syn::Error::new(
        Span::call_site(),
        format!(
            "cannot lock identifier artefact directory {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
            root.display()
        ),
    )
}

/// Acquires the exclusive publication lock for one artefact directory.
///
/// Blocks in the kernel until a peer process releases the lock. Returns the
/// held guard; the caller must keep it alive for the whole fragment-write,
/// merge, and publication sequence.
///
/// This is the quiet counterpart to [`lock_while`], taken when the observation
/// channel is off. Nothing would read a wait announcement there, so a real
/// blocking wait is both cheaper than polling and sufficient.
pub(super) fn lock(root: &Path) -> syn::Result<LockGuard> {
    let file = open_lock_file(&lock_path(root))?;
    file.lock().map_err(|error| lock_error(root, &error))?;
    Ok(LockGuard { file })
}

/// Acquires the lock, reporting each observed wait to `on_wait`.
///
/// `on_wait` receives the time spent waiting so far, once per [`POLL_INTERVAL`]
/// while the lock stays contended, so a caller can surface a prolonged wait
/// before the lock is ever granted. It is called only when the lock is not
/// immediately free.
pub(super) fn lock_while(root: &Path, mut on_wait: impl FnMut(Duration)) -> syn::Result<LockGuard> {
    let file = open_lock_file(&lock_path(root))?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(LockGuard { file }),
            Err(fs::TryLockError::WouldBlock) => {
                on_wait(started.elapsed());
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(fs::TryLockError::Error(error)) => return Err(lock_error(root, &error)),
        }
    }
}

/// Brings the lock file into existence without taking the lock.
///
/// Ordinary acquisition creates the file as a side effect of opening it for
/// writing. A test-only probe must not rely on that: it opens the existing
/// file, so the file has to be created deliberately, and doing so here keeps
/// that creation an explicit command rather than a mutation hidden inside a
/// query.
#[cfg(test)]
pub(super) fn create_lock(root: &Path) -> std::io::Result<()> {
    drop(create_lock_file(&lock_path(root))?);
    Ok(())
}

/// The outcome of one non-blocking observation of the publication lock.
///
/// A contended probe carries the metadata of the handle that observed the
/// contention, so a caller can name the file it could not lock rather than
/// only the predicate it disproved. Answering "held" without saying which
/// file was held is what left the intermittent post-release failures
/// undiagnosable: the same boolean is produced by a leaked handle of our own
/// and by an unrelated holder of the same inode.
#[cfg(test)]
#[derive(Debug)]
pub(super) enum LockProbe {
    /// No holder contended with the probe, so the lock is free.
    Free,
    /// Something still owned the lock when the probe tried to take it.
    Held {
        /// Metadata for the lock file, read through the contending handle.
        ///
        /// Captured on the contention path only: a free probe must not pay an
        /// extra system call to answer a question the acquisition already
        /// answered.
        metadata: fs::Metadata,
    },
}

/// Observes the publication lock without blocking, retaining what it observed.
///
/// Never blocks: a held lock is reported as [`LockProbe::Held`] rather than
/// waited for, so callers can surface a stuck peer instead of hanging. This
/// exists so lock regression tests can observe the lock's state through the
/// same path the production code locks, rather than duplicating the filename.
///
/// The probe is read-only by construction. It opens without create or
/// truncate and explicitly unlocks a successful non-blocking acquisition
/// before returning. This prevents a concurrently forked child from keeping
/// the probe's lock after the parent closes its handle. An observation
/// therefore cannot create the file, clear its contents, or keep the lock it
/// just took. It answers with the raw I/O error rather than
/// `open_lock_file`'s spanned diagnostic, and a missing lock file is
/// `NotFound` — call [`create_lock`] to set the probe up.
///
/// The read-only handle is deliberate even though [`fs::File::try_lock`]
/// documents that "if the file is not open for writing, it is unspecified
/// whether this function returns an error". Opening for writing would make
/// the probe mutate the file's metadata and would let an observation create
/// or truncate it, which is exactly what this probe must not do. On Linux the
/// documented `flock`-backed implementation acquires an exclusive lock on a
/// read-only descriptor without complaint; the portability caveat is accepted
/// for a test-only probe rather than paid for with a write handle.
#[cfg(test)]
pub(super) fn probe_lock(root: &Path) -> std::io::Result<LockProbe> {
    let file = open_existing_lock_file(&lock_path(root))?;
    let probe = match file.try_lock() {
        Ok(()) => {
            // Released explicitly for the reason the production guard records:
            // a concurrent fork inherits the open file description before exec
            // closes its descriptor, and the child would otherwise hold a lock
            // this probe has already finished observing.
            file.unlock()?;
            LockProbe::Free
        }
        Err(fs::TryLockError::WouldBlock) => LockProbe::Held {
            metadata: file.metadata()?,
        },
        Err(fs::TryLockError::Error(error)) => return Err(error),
    };
    // The failed acquisition in the contended arm left nothing to release, so
    // closing the handle is enough there.
    drop(file);
    Ok(probe)
}

/// Reports whether the publication lock is currently free.
///
/// The boolean view of [`probe_lock`], kept for callers that only assert the
/// lock's state. A caller that needs to say *why* a released lock still looks
/// held should call `probe_lock` — or, better, the diagnostics in
/// `artefact_lock_diagnostics` — so the failure names a holder.
#[cfg(test)]
pub(super) fn is_free(root: &Path) -> std::io::Result<bool> {
    Ok(matches!(probe_lock(root)?, LockProbe::Free))
}

#[cfg(all(test, unix))]
mod lock_tests {
    //! Verify guard drop releases the lock while a duplicate descriptor stays open.

    use anyhow::{Context, Result, ensure};
    use rstest::rstest;

    use super::super::fragment_tests::TempRoot;
    use super::{create_lock, is_free, lock};

    #[rstest]
    fn dropping_guard_unlocks_while_a_duplicate_descriptor_remains_open() -> Result<()> {
        let root = TempRoot::new()?;
        create_lock(root.path()).context("create the lock file for the duplicate test")?;
        let guard = lock(root.path())?;
        let duplicate = guard
            .file
            .try_clone()
            .context("duplicate the held lock descriptor")?;

        drop(guard);

        ensure!(
            is_free(root.path()).context("probe while the duplicate descriptor remains open")?,
            "dropping the guard must unlock before its duplicate descriptor closes",
        );
        drop(duplicate);
        Ok(())
    }
}
// weave: run 'weave explain ortho_config_macros/src/derive/generate/localization/artefact/artefact_lock.rs' for per-hunk detail, 'weave check' to verify your resolution
