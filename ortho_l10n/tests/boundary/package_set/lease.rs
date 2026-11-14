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

/// Name of the record naming the run whose roots are materialized.
const COMPLETION_FILE_NAME: &str = ".packaging-complete";

/// Environment variable nextest exports to every test it runs.
const RUN_ID_VARIABLE: &str = "NEXTEST_RUN_ID";

/// How often a blocked contract re-checks the lease and the record.
///
/// The packaging step is measured in hundreds of seconds, so polling this
/// often costs nothing measurable while keeping the hand-off prompt.
pub(crate) const LEASE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// A lease held for the whole of the packaging step.
///
/// The lock lives exactly as long as this value; dropping it releases the
/// lease, and so does the death of the process that holds it. The packaging
/// target is rewritten beneath it, so the handle is carried for the whole step
/// rather than dropped after the check.
pub(crate) struct PackagingLease {
    /// The locked file. Never read; kept so the kernel lock outlives the call.
    _file: File,
}

/// What a contract found when it arrived at the packaging step.
pub(crate) enum Prepared {
    /// The roots were already materialized by this run; package nothing.
    Reused,
    /// This contract owns the step. The lease must be held until the roots are
    /// whole, and `directory` is the target it protects.
    Owned {
        /// The held lease, released when this value drops.
        lease: PackagingLease,
        /// The packaging target, for the record beside the roots.
        directory: Dir,
    },
}

/// Record that this run's packaged set is complete.
///
/// A free function rather than a method on [`PackagingLease`], though it must
/// only ever be called while that lease is held. The lease cannot enforce the
/// ordering in its type without a lifetime parameter threaded through every
/// caller, and a `&self` it never reads is a lint violation rather than a
/// guarantee. The ordering is stated where the guard actually lives, in
/// [`super::roots`]'s packaging step: the record is written inside the
/// `Prepared::Owned` arm, before the lease is dropped.
///
/// A reader can therefore see a partial record only if this call raced the
/// release of the lease, which the exclusive lock makes impossible: the write
/// happens while the lock is held, and the lock is released after it.
///
/// # Errors
///
/// Returns an error when the record cannot be written.
pub(crate) fn record_complete(directory: &Dir, scope: &str) -> Result<()> {
    directory
        .write(COMPLETION_FILE_NAME, scope)
        .context("record that the packaged set is complete")
}

/// The value the completion record must name for this process to reuse it.
///
/// `None` when no run identifier is exported, which is the single-process
/// runners: there the process-local cache is what deduplicates, and a record
/// made by an earlier run must not be trusted.
pub(crate) fn run_scope() -> Option<String> {
    std::env::var(RUN_ID_VARIABLE)
        .ok()
        .filter(|id| !id.is_empty())
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
/// cannot be locked, or when `deadline` passes while another contract still
/// holds it.
pub(crate) fn prepare(
    target: &Utf8Path,
    scope: Option<&str>,
    deadline: Instant,
) -> Result<Prepared> {
    Dir::create_ambient_dir_all(target, cap_std::ambient_authority())
        .with_context(|| format!("create the packaging target {target}"))?;
    let directory = Dir::open_ambient_dir(target, cap_std::ambient_authority())
        .with_context(|| format!("open the packaging target {target}"))?;
    loop {
        if record_matches(&directory, scope) {
            return Ok(Prepared::Reused);
        }
        match try_lease(&directory, target) {
            Ok(lease) => {
                if record_matches(&directory, scope) {
                    return Ok(Prepared::Reused);
                }
                return Ok(Prepared::Owned { lease, directory });
            }
            Err(error) if is_held_elsewhere(&error) => {}
            Err(error) => {
                return Err(error).context(format!("lease the packaging target {target}"));
            }
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

/// Whether the completion record names this run.
///
/// A missing or unreadable record reads as "not complete": the roots are then
/// packaged again rather than trusted on the strength of a file that cannot be
/// read. With no scope the answer is always `false`, because a record written
/// by an earlier run says nothing about this process's sources.
fn record_matches(directory: &Dir, scope: Option<&str>) -> bool {
    let Some(this_run) = scope else {
        return false;
    };
    directory
        .read_to_string(COMPLETION_FILE_NAME)
        .is_ok_and(|record| record == this_run)
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
        Ok(()) => Ok(PackagingLease { _file: file }),
        Err(TryLockError::WouldBlock) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Open the lease file with no sharing permitted.
#[cfg(windows)]
fn try_lease(_directory: &Dir, target: &Utf8Path) -> io::Result<PackagingLease> {
    use std::os::windows::fs::OpenOptionsExt;

    // `share_mode(0)` denies read, write and delete to every other open,
    // including one from this process. The file is therefore created if it is
    // absent and opened exclusively if it is not.
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .share_mode(0)
        .open(target.join(LEASE_FILE_NAME).as_std_path())
        .map(|file| PackagingLease { _file: file })
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

/// Drop the completion record before the roots are replaced.
///
/// Called under the lease, before the first packaging invocation. Without it a
/// run could record a set, lose a root, and have a later contract in the same
/// run read the record as proof that the set is whole. Removing it first makes
/// the invariant local: the record exists only between a successful pair and
/// the next packaging step, and a failed step leaves none behind.
pub(crate) fn clear_record(directory: &Dir) {
    let _removed = directory.remove_file(COMPLETION_FILE_NAME);
}
