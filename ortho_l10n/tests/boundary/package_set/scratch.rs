//! A throwaway packaging target for the lease and record regressions.
//!
//! Both halves of the packaging bookkeeping - taking the lease, and writing or
//! clearing the completion record - need the same thing to be tested against: a
//! directory that looks like the real packaging target, carries a real kernel
//! lock, and disappears afterwards. Sharing one fixture keeps the two test
//! modules from drifting into slightly different notions of "a packaging
//! target", which would let a change pass in one and fail in the other for
//! reasons that have nothing to do with the code under test.
//!
//! The directory is created inside the boundary root the suite already uses,
//! rather than in the system temporary directory, so the lease is taken on the
//! same filesystem as the real one. A cross-device rename or an exotic mount
//! option is not something these cases should be discovering.

use super::lease::{PackagingLease, Prepared, prepare};
use crate::boundary::fixture_cargo::{BOUNDARY_ROOT, repository_root};
use anyhow::{Context, Result};
use cap_std::fs_utf8::Dir;
use rstest::fixture;
use std::time::{Duration, Instant};

/// A deadline far enough away that nothing here waits on it.
pub(crate) const AMPLE_DEADLINE: Duration = Duration::from_secs(60);

/// The longest a waiter may take to pick up a free lease.
///
/// A waiter re-checks the lease every `LEASE_POLL_INTERVAL`, so the hand-off
/// after a release costs at most one interval plus scheduling noise. The bound
/// is generous for the noise rather than tight for the interval: it fails a
/// waiter that sleeps for the packaging deadline, not one that was descheduled
/// for a moment.
pub(crate) const WAKE_UP_BOUND: Duration = Duration::from_secs(2);

/// A scratch packaging target, removed when the value drops.
pub(crate) struct Scratch {
    /// The drop guard. Never read; it removes the directory.
    _tempdir: tempfile::TempDir,
    /// The target as a UTF-8 path, which the lease takes.
    path: camino::Utf8PathBuf,
}

/// The `#[rstest]` fixture form, for the cases that take it as a parameter.
///
/// A thin wrapper rather than the attribute on [`new_scratch`] itself, because
/// the `proptest` case has to call the constructor imperatively: a fixture
/// stands in for a test parameter, and a property's body is not one.
#[fixture]
pub(crate) fn scratch_root() -> Result<Scratch> {
    new_scratch()
}

/// Create an empty packaging target for one regression.
///
/// Fallible rather than panicking: the boundary root may not exist and
/// `tempfile` may refuse the allocation, and neither is a defect in the code
/// these cases are about.
///
/// # Errors
///
/// Returns an error when the boundary root cannot be created or when the
/// temporary directory cannot be allocated.
pub(crate) fn new_scratch() -> Result<Scratch> {
    let repository = repository_root()?;
    let boundary = repository.join(BOUNDARY_ROOT);
    Dir::create_ambient_dir_all(&boundary, cap_std::ambient_authority())
        .with_context(|| format!("create boundary root {boundary}"))?;
    let tempdir = tempfile::Builder::new()
        .prefix("lease-")
        .tempdir_in(&boundary)
        .context("create a scratch packaging target")?;
    let path = camino::Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("the scratch root {} is not UTF-8", path.display()))?;
    Ok(Scratch {
        _tempdir: tempdir,
        path,
    })
}

impl Scratch {
    /// The target's path, for the calls that take one directly.
    pub(crate) fn path(&self) -> &camino::Utf8Path {
        &self.path
    }

    /// The target as a directory handle, for the calls that read a record
    /// without holding a lease.
    ///
    /// # Errors
    ///
    /// Returns an error when the target cannot be opened.
    pub(crate) fn open(&self) -> Result<Dir> {
        Dir::open_ambient_dir(&self.path, cap_std::ambient_authority())
            .with_context(|| format!("open the scratch packaging target {}", self.path))
    }

    /// Take the lease in this process, failing the case if it is not granted.
    ///
    /// Returned rather than dropped so the caller decides when it is released.
    ///
    /// # Errors
    ///
    /// Returns an error when the lease is not granted, including when a fresh
    /// target with no record was reported as already packaged.
    pub(crate) fn take(&self, scope: Option<&str>) -> Result<PackagingLease> {
        match prepare(&self.path, scope, Instant::now() + AMPLE_DEADLINE)? {
            Prepared::Owned { lease } => Ok(lease),
            Prepared::Reused => anyhow::bail!(
                "a fresh target with no record was reported as already packaged; \
                 a missing record must read as not complete"
            ),
        }
    }
}
