//! The record and clear sequence that runs under the packaging lease.
//!
//! [`package_under_lease`] is the only place the completion record is written,
//! and the order it enforces is the whole reason the lease exists: clear before
//! packaging, record only after it succeeds. Both halves are invisible from the
//! packaged contracts, which only see whether roots are present, so a mistake
//! here would be caught by nothing but a slow, intermittently wrong run.
//!
//! These cases drive the sequence with a closure standing in for `cargo
//! package`. That keeps them fast and lets them make packaging fail on demand,
//! which is the case the record must not survive.

use crate::boundary::fixture_cargo::{BOUNDARY_ROOT, repository_root};
use crate::boundary::package_set::lease::{PackagingLease, Prepared, prepare};
use crate::boundary::package_set::record::{read_unlocked, record_complete};
use crate::boundary::package_set::roots::package_under_lease;
use anyhow::{Context, Result, ensure};
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};
use std::time::{Duration, Instant};

/// A deadline far enough away that nothing here waits on it.
const AMPLE_DEADLINE: Duration = Duration::from_secs(60);

/// The run identifier these cases record under.
const THIS_RUN: &str = "roots-regression-this-run";

/// A scratch packaging target with a lease already taken.
///
/// The fields are ordered for their destructors, not for the reader. Rust
/// drops them in declaration order, so declaring the lease first is what makes
/// it the first to go: its file handle closes and the lock is released before
/// the scratch tree is removed. The reverse order leaves the lock held while
/// `TempDir` tries to delete the directory, and on Windows that deletion is
/// denied by the lease's own share mode - silently, because `TempDir` ignores
/// cleanup errors, so the case would pass while leaving the tree behind.
struct Held {
    /// The lease, held for the whole of the sequence under test.
    lease: PackagingLease,
    /// Owns the scratch directory, removing it when the case ends.
    tempdir: tempfile::TempDir,
}

/// Take a lease on a fresh target, failing the case if it is not granted.
///
/// Fallible fixture: allocating the temporary directory, opening it and
/// acquiring the lease can each fail, and a shared fixture should report that
/// rather than panic every case that uses it.
#[fixture]
fn held_lease() -> Result<Held> {
    let boundary = repository_root()?.join(BOUNDARY_ROOT);
    Dir::create_ambient_dir_all(&boundary, cap_std::ambient_authority())
        .with_context(|| format!("create the boundary root {boundary}"))?;
    let tempdir = tempfile::Builder::new()
        .prefix("roots-")
        .tempdir_in(&boundary)
        .context("create a scratch packaging target")?;
    let path = camino::Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("the scratch target {} is not UTF-8", path.display()))?;

    let lease = match prepare(&path, Some(THIS_RUN), Instant::now() + AMPLE_DEADLINE)? {
        Prepared::Owned { lease } => lease,
        Prepared::Reused => anyhow::bail!(
            "a fresh target with no record was reported as already packaged, so \
             the sequence below would not run at all"
        ),
    };
    Ok(Held { lease, tempdir })
}

/// The directory the held lease protects.
fn directory(held: &Held) -> Result<Dir> {
    let path = camino::Utf8Path::from_path(held.tempdir.path())
        .context("the scratch packaging target is not UTF-8")?;
    Dir::open_ambient_dir(path, cap_std::ambient_authority())
        .context("open the scratch packaging target")
}

/// A successful step records the run, so the next contract reuses the roots.
#[rstest]
fn a_successful_step_records_the_run(held_lease: Result<Held>) -> Result<()> {
    let held = held_lease?;
    package_under_lease(&held.lease, Some(THIS_RUN), || Ok(()))?;

    ensure!(
        read_unlocked(&directory(&held)?, Some(THIS_RUN))?,
        "a packaging step that succeeded left no record, so every later \
         contract in the run would package the same sources again"
    );
    Ok(())
}

/// A failed step leaves no record, so no contract reads a partial set.
#[rstest]
fn a_failed_step_leaves_no_record(held_lease: Result<Held>) -> Result<()> {
    let held = held_lease?;

    // Seed a record, so the case proves the clear happened rather than that
    // nothing had been written yet.
    record_complete(&held.lease, THIS_RUN)?;
    ensure!(
        read_unlocked(&directory(&held)?, Some(THIS_RUN))?,
        "the seeded record was not readable, so this case proves nothing"
    );

    let failed = package_under_lease(&held.lease, Some(THIS_RUN), || {
        anyhow::bail!("the second packaging invocation failed")
    });
    ensure!(
        failed.is_err(),
        "a failing packaging step was reported as a success"
    );
    ensure!(
        !read_unlocked(&directory(&held)?, Some(THIS_RUN))?,
        "a failed packaging step left the completion record in place, so a \
         later contract would read a half-replaced set as whole"
    );
    Ok(())
}

/// With no run scope nothing is recorded; the process-local cache suffices.
#[rstest]
fn without_a_run_scope_nothing_is_recorded(held_lease: Result<Held>) -> Result<()> {
    let held = held_lease?;
    package_under_lease(&held.lease, None, || Ok(()))?;

    ensure!(
        !read_unlocked(&directory(&held)?, Some(THIS_RUN))?,
        "a record was written with no run scope, so an earlier run's roots \
         could later be read as this process's work"
    );
    Ok(())
}

/// The record is cleared before the packaging step starts, not after it.
///
/// The ordering is what stops a contract that loses a root from leaving the
/// record describing a set that is no longer whole. Observed from inside the
/// packaging closure, which is the only vantage point that can see whether the
/// clear has already happened.
#[rstest]
fn the_record_is_cleared_before_packaging_starts(held_lease: Result<Held>) -> Result<()> {
    let held = held_lease?;
    record_complete(&held.lease, THIS_RUN)?;

    let mut seen = None;
    package_under_lease(&held.lease, Some(THIS_RUN), || {
        seen = Some(read_unlocked(&directory(&held)?, Some(THIS_RUN))?);
        Ok(())
    })?;

    ensure!(
        seen == Some(false),
        "the record was still present when packaging began, so a step that \
         replaced the roots would leave a record describing the old ones"
    );
    Ok(())
}

/// The record is written only after the packaging step has finished.
///
/// The other half of the ordering, observed from inside the closure: the clear
/// has happened and the write has not. Together with
/// [`the_record_is_cleared_before_packaging_starts`] this pins the sequence as
/// clear, package, record - a step that recorded *before* packaging would be
/// indistinguishable from a correct one from the outside, because both leave a
/// record and freshly built roots.
#[rstest]
fn the_record_is_not_written_until_packaging_finishes(held_lease: Result<Held>) -> Result<()> {
    let held = held_lease?;

    let mut during = None;
    package_under_lease(&held.lease, Some(THIS_RUN), || {
        during = Some(read_unlocked(&directory(&held)?, Some(THIS_RUN))?);
        Ok(())
    })?;

    ensure!(
        during == Some(false),
        "the record was already present while packaging was still running, so a \
         later contract could read a half-built set as whole"
    );
    ensure!(
        read_unlocked(&directory(&held)?, Some(THIS_RUN))?,
        "the record was not written once packaging finished, so the next \
         contract in the run would package the same sources again"
    );
    Ok(())
}
