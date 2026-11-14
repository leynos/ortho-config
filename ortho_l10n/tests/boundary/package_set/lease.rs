//! Serializes the packaging step across every test process in one run.
//!
//! `cargo package --verify` is expensive: it compiles each packaged crate's
//! whole dependency table from an unpacked copy of its own archive, which on a
//! shared host can spend hundreds of seconds waiting for compile admission
//! before it finishes. The suite materializes three crates, so one packaging
//! pair is two invocations, and on a loaded host that pair does not fit a
//! per-test bound that also leaves room for the contracts that read the roots.
//!
//! `prepare_packaged_set` caches its result per *process*, but nextest runs
//! each test in a process of its own. Every packaged contract therefore
//! re-packages the same three crates into the same target directory, and six
//! contracts make that up to twelve invocations of identical work. The
//! `boundary_packaging` test group serializes the binary, which removes the
//! *concurrent* race but not the repetition: the packaging still runs once per
//! contract, sequentially.
//!
//! This module removes the repetition. A lease file beside the packaging
//! target is locked exclusively for the whole of the packaging step, and the
//! unpacked roots are recorded as complete only once both invocations have
//! succeeded. A contract that finds the roots already complete for its run
//! reads them without packaging anything.
//!
//! # The record is scoped to one run
//!
//! Reuse is deliberately never wider than a single test run. nextest exports
//! `NEXTEST_RUN_ID` to every test process, so the record names that run and a
//! later run finds it foreign and packages afresh. That keeps the suite's
//! existing behaviour - each run materializes the roots from the sources as
//! they are - while collapsing the twelve invocations of one run to two.
//!
//! Widening the reuse to the target directory's lifetime would let a checkout
//! that has since changed be read through roots built from older sources, and
//! nothing here can cheaply prove that it has not. A per-run record needs no
//! such proof, because the run that reads the roots is the run that wrote them.
//!
//! When no run identifier is exported - `cargo test` running the binary's
//! tests as threads in one process - no record is written and none is
//! consulted, so the lease degrades to a plain mutex over the packaging step.
//! [`super::roots`]'s process-local cache already provides the deduplication
//! there, and the lease keeps even a direct call to the packaging step from
//! running beside another process.
//!
//! # What this module owns, and what it does not
//!
//! Only acquisition: the lease, the wait for it, and the deadline that bounds
//! the wait. The *contents* of the record - how it is written, cleared and
//! read - live in [`super::record`], which is also where the two-invariants
//! argument for taking the lease as a precondition is made. The run identifier
//! is read by [`super::roots`] and passed in, so this module consults no
//! environment variable.
//!
//! # Why a file lock
//!
//! `cap-std` exposes no locking API, so the lease is the standard library's
//! `flock` (Unix) or share-mode open (Windows). It is exempted, as a single
//! module, from the capability-based filesystem policy in `dylint.toml`, on
//! the same terms as the probe's descendant lock: the only file it touches is
//! its own lease inside the boundary's private target, and the only thing it
//! protects is that target.
//!
//! The lease is held for the packaging step alone, never while a contract
//! reads the roots, so readers never block on it once the roots exist.

#[cfg(test)]
#[path = "lease_tests.rs"]
mod lease_tests;

#[cfg(test)]
#[path = "lease_shape_tests.rs"]
mod lease_shape_tests;

use super::record::{read_under_lease, read_unlocked};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use std::fs::File;
use std::io;
use std::time::{Duration, Instant};

/// Name of the lease file, inside the packaging target.
///
/// The file shares the target's lifetime. Its contents are irrelevant: the
/// lease is the kernel-held lock on it, released when the owning handle closes
/// or the process dies.
pub(crate) const LEASE_FILE_NAME: &str = ".packaging.lease";

/// How often a blocked contract re-checks the lease and the record.
///
/// The packaging step is measured in hundreds of seconds, so polling this
/// often costs nothing measurable while keeping the hand-off prompt. The bound
/// is pinned by `the_lease_poll_interval_stays_a_handoff`, and the hand-off it
/// promises is pinned by `a_waiter_wakes_promptly_after_the_holder_releases`.
pub(crate) const LEASE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// A lease held for the whole of the packaging step.
///
/// The lock lives exactly as long as this value; dropping it releases the
/// lease, and so does the death of the process that holds it. The packaging
/// target is rewritten beneath it, so the handle is carried for the whole step
/// rather than dropped after the check.
///
/// The target is carried too, rather than passed alongside, so the record
/// helpers can take the lease itself and have the directory they write under.
/// Holding a lease and naming a different directory is then not expressible.
pub(crate) struct PackagingLease {
    /// The locked file. Never read; kept so the kernel lock outlives the call.
    _file: File,
    /// The packaging target the lease protects.
    directory: Dir,
}

impl PackagingLease {
    /// The packaging target this lease protects.
    ///
    /// `pub(super)` rather than private: [`super::record`] takes the lease as
    /// the proof that the lock is held, and needs the directory it protects to
    /// write, clear and read the record under it. It stays inside the
    /// `package_set` boundary, so no contract can reach it.
    pub(super) const fn directory(&self) -> &Dir {
        &self.directory
    }
}

/// What a contract found when it arrived at the packaging step.
pub(crate) enum Prepared {
    /// The roots were already materialized by this run; package nothing.
    Reused,
    /// This contract owns the step. The lease must be held until the roots are
    /// whole, and it names the target those roots live under.
    Owned {
        /// The held lease, released when this value drops.
        lease: PackagingLease,
    },
}

/// Take the packaging lease, or report that this run's roots are complete.
///
/// The wait for a lease held by another contract is bounded by `deadline`, the
/// end of the calling contract's shared allowance, and a wait that outlives it
/// is reported as a wait rather than as a bare timeout. Once the lease is
/// acquired the record is re-checked, because the contract that held it may
/// have completed the step while this one waited.
///
/// # Why the wait is bounded by the allowance and nothing else
///
/// A reserve carved out of the deadline - enough for the packaging that follows
/// a successful wait - would couple two quantities that move independently: how
/// long a wait may last, and how long packaging may take. It would silently
/// under-protect whenever a packaging invocation or its count changed. The
/// existing machinery already covers the case a reserve would guard against:
/// a contract whose allowance is spent by the wait hands its packaging call to
/// [`super::super::fixture_cargo::Budget::bound`], which refuses it and names
/// the invocation that could not start. [`super::SCENARIO_ALLOWANCE`] is inside
/// nextest's per-test allowance precisely so that error is printed before
/// nextest stops the test, so the wait is never reported as a bare timeout.
///
/// # Errors
///
/// Returns an error when the target cannot be created or opened, when the lease
/// cannot be locked, when the record cannot be read, or when `deadline` passes
/// while another contract still holds it.
pub(crate) fn prepare(
    target: &Utf8Path,
    scope: Option<&str>,
    deadline: Instant,
) -> Result<Prepared> {
    let directory = open_target(target)?;
    loop {
        // The arrival check, before any lease exists. `read_unlocked` takes the
        // directory because this caller has no proof to offer.
        if read_unlocked(&directory, scope)? {
            return Ok(Prepared::Reused);
        }
        if let Some(prepared) = claim(&directory, scope, target)? {
            return Ok(prepared);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !left.is_zero(),
            "another boundary test process held the packaging lease {} until this \
             contract's shared allowance ran out; that process was packaging the same \
             sources, so this contract stopped rather than packaging them a second \
             time beside it",
            target.join(LEASE_FILE_NAME),
        );
        std::thread::sleep(LEASE_POLL_INTERVAL.min(left));
    }
}

/// Open the packaging target, creating it only if it is not there yet.
///
/// The distinction matters on the reuse path, which is supposed to touch
/// nothing: a contract that arrives at a target that already exists makes no
/// change to it, so the conditional create is what keeps the reuse path free of
/// writes. Opening has to come first because the record lives *inside* the
/// directory - a reuse check that ran before opening it would have nothing to
/// read. What can be ordered the other way round is the create, and that is
/// what this does. The target is shared by every contract in the run, so a
/// neighbour having created it is the ordinary case rather than a race to
/// report.
///
/// # Errors
///
/// Returns an error when the target exists but cannot be opened, or when it is
/// absent and cannot be created.
fn open_target(target: &Utf8Path) -> Result<Dir> {
    match Dir::open_ambient_dir(target, cap_std::ambient_authority()) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Dir::create_ambient_dir_all(target, cap_std::ambient_authority())
                .with_context(|| format!("create the packaging target {target}"))?;
            Dir::open_ambient_dir(target, cap_std::ambient_authority())
                .with_context(|| format!("open the packaging target {target}"))
        }
        Err(error) => Err(error).with_context(|| format!("open the packaging target {target}")),
    }
}

/// Try to take the lease, reporting whether this contract now owns the step.
///
/// Returns `Ok(None)` when another process holds the lease, which asks the
/// caller to keep waiting rather than to report a failure. `Ok(Some(..))` means
/// the contract either owns the packaging step or found the roots already
/// complete for this run — the second check exists because the holder may have
/// finished between the caller's first check and the moment the lock was won.
///
/// # Errors
///
/// Returns an error when the lease cannot be taken for any reason other than
/// another process holding it, or when the record cannot be read under it.
fn claim(directory: &Dir, scope: Option<&str>, target: &Utf8Path) -> Result<Option<Prepared>> {
    match try_lease(directory, target) {
        Ok(lease) => Ok(Some(if read_under_lease(&lease, scope)? {
            Prepared::Reused
        } else {
            Prepared::Owned { lease }
        })),
        Err(error) if is_held_elsewhere(&error) => Ok(None),
        Err(error) => Err(error).context(format!("lease the packaging target {target}")),
    }
}

/// Open the lease file and take its lock without waiting.
///
/// Unix takes the lease through a capability handle and relies on `flock`, so
/// the open itself never waits. Windows has to deny sharing at open time,
/// because `LockFileEx` locks belong to the locking process rather than to the
/// file object; an open that is refused is therefore reported as the caller's
/// "held elsewhere" case.
#[cfg(not(windows))]
fn try_lease(directory: &Dir, _target: &Utf8Path) -> io::Result<PackagingLease> {
    use std::fs::TryLockError;

    // `Dir::create` truncates, which is harmless here: the lease is the lock on
    // the file, and the file's contents are never read. It is also the shortest
    // route to a handle the standard library will lock, since `fs_utf8`'s
    // `Dir::open_with` takes that module's own `OpenOptions`.
    let file = directory
        .create(LEASE_FILE_NAME)
        .map(cap_std::fs_utf8::File::into_std)?;
    match file.try_lock() {
        Ok(()) => Ok(PackagingLease {
            _file: file,
            directory: directory.try_clone()?,
        }),
        Err(TryLockError::WouldBlock) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Open the lease file with no sharing permitted.
#[cfg(windows)]
fn try_lease(directory: &Dir, target: &Utf8Path) -> io::Result<PackagingLease> {
    use std::os::windows::fs::OpenOptionsExt;

    // `share_mode(0)` denies read, write and delete to every other open,
    // including one from this process. The file is therefore created if it is
    // absent and opened exclusively if it is not.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .share_mode(0)
        .open(target.join(LEASE_FILE_NAME).as_std_path())?;
    Ok(PackagingLease {
        _file: file,
        directory: directory.try_clone()?,
    })
}

/// Whether an open or lock failed because another process holds the lease.
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
