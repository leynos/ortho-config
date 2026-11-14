//! Regressions for the packaging lease and the packaging boundary's shape.
//!
//! The lease exists so that one nextest *run* performs one packaging pair, not
//! one per test process. Three separate properties have to hold for that to be
//! true, and each has its own failure mode, so each has its own test:
//!
//! - a record written for this run makes the next caller reuse the roots;
//! - a record written for a *different* run does not, because the roots may be
//!   stale;
//! - a lease held elsewhere is reported as a wait, never as a silent second
//!   packaging step.
//!
//! The fourth test is structural rather than behavioural: it pins how many
//! packaging invocations the suite performs per run, so an edit that returns
//! to per-test packaging fails here rather than only making CI slow. It reads
//! the source because the property is about the *shape* of the code - how many
//! call sites reach the packaging step - and no runtime observation of one
//! contract can see a second call site in another module.

use crate::boundary::fixture_cargo::{BOUNDARY_ROOT, repository_root};
use crate::boundary::package_set::lease::{
    LEASE_FILE_NAME, LEASE_POLL_INTERVAL, PackagingLease, Prepared, clear_record, prepare,
    record_complete,
};
use crate::boundary::package_set::{PACKAGED_CRATES, PACKAGING_INVOCATIONS};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};
use std::time::{Duration, Instant};

/// A deadline far enough away that no test here waits on it.
const AMPLE_DEADLINE: Duration = Duration::from_secs(60);

/// A deadline already in the past, so a held lease is reported at once.
const SPENT_DEADLINE: Duration = Duration::ZERO;

/// The run identifier these tests record under.
///
/// Not `NEXTEST_RUN_ID`: a real run's value would be visible to the other
/// contracts in the same binary and could make one of them reuse roots this
/// test's scratch directory does not hold.
const THIS_RUN: &str = "lease-regression-this-run";

/// A different run identifier, standing for an earlier run's record.
const OTHER_RUN: &str = "lease-regression-another-run";

/// A scratch packaging target, removed when the test ends.
struct Scratch {
    /// The drop guard. Never read; it removes the directory.
    _tempdir: tempfile::TempDir,
    /// The directory as a UTF-8 path, which the lease takes.
    path: camino::Utf8PathBuf,
}

/// Create an empty packaging target for one regression.
///
/// Fallible fixture so a test can apply `?` to it: the boundary root may not
/// exist and `tempfile` may refuse the allocation, and neither should panic a
/// fixture that is shared by every case here.
#[fixture]
fn scratch_root() -> Result<Scratch> {
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
    /// The directory, for the lease calls that take one.
    fn open(&self) -> Result<Dir> {
        Dir::open_ambient_dir(&self.path, cap_std::ambient_authority())
            .with_context(|| format!("open the scratch packaging target {}", self.path))
    }

    /// Take the lease in this process, failing the test if it is not granted.
    ///
    /// Returned rather than dropped so a caller decides when it is released.
    fn take(&self) -> Result<PackagingLease> {
        match prepare(&self.path, Some(THIS_RUN), Instant::now() + AMPLE_DEADLINE)? {
            Prepared::Owned { lease, .. } => Ok(lease),
            Prepared::Reused => anyhow::bail!(
                "a fresh target with no record was reported as already packaged; \
                 a missing record must read as not complete"
            ),
        }
    }
}

/// A record for this run makes the roots reusable without packaging.
#[rstest]
fn a_record_for_this_run_is_reused(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let lease = scratch.take()?;
    record_complete(&scratch.open()?, THIS_RUN)?;
    drop(lease);

    let prepared = prepare(
        &scratch.path,
        Some(THIS_RUN),
        Instant::now() + AMPLE_DEADLINE,
    )?;
    ensure!(
        matches!(prepared, Prepared::Reused),
        "a record naming this run was not reused, so every contract in the run \
         would package again"
    );
    Ok(())
}

/// A record for another run must not be reused.
///
/// This is the property that keeps the reuse from spanning runs. The roots in
/// the target may have been built from sources that have since changed, and
/// nothing here can cheaply prove otherwise, so a foreign record reads as "not
/// complete" and this run packages afresh.
#[rstest]
fn a_record_for_another_run_is_not_reused(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let lease = scratch.take()?;
    record_complete(&scratch.open()?, OTHER_RUN)?;
    drop(lease);

    let prepared = prepare(
        &scratch.path,
        Some(THIS_RUN),
        Instant::now() + AMPLE_DEADLINE,
    )?;
    let Prepared::Owned { .. } = prepared else {
        anyhow::bail!(
            "a record naming {OTHER_RUN} was reused for {THIS_RUN}; roots built by \
             an earlier run may not describe this run's sources"
        );
    };
    Ok(())
}

/// With no run identifier, no record is written and none is trusted.
///
/// `cargo test` and `make boundary` export no `NEXTEST_RUN_ID`. The
/// process-local cache deduplicates there, and a record left by an earlier
/// nextest run must not be mistaken for this process's work.
#[rstest]
fn without_a_run_scope_a_record_is_neither_written_nor_trusted(
    scratch_root: Result<Scratch>,
) -> Result<()> {
    let scratch = scratch_root?;
    let lease = scratch.take()?;
    record_complete(&scratch.open()?, THIS_RUN)?;
    drop(lease);

    let prepared = prepare(&scratch.path, None, Instant::now() + AMPLE_DEADLINE)?;
    let Prepared::Owned { .. } = prepared else {
        anyhow::bail!(
            "with no run scope a record naming {THIS_RUN} was reused; the \
             single-process runners must package rather than trust it"
        );
    };
    Ok(())
}

/// A lease held by another handle is reported as a wait, not taken again.
///
/// The second attempt is bounded by an already-spent deadline, so it must fail
/// rather than block. That is the shape a real second process sees: it cannot
/// acquire the lease, and it must say it waited instead of silently packaging
/// beside the holder.
#[rstest]
fn a_held_lease_is_reported_as_a_wait(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let held = scratch.take()?;

    let refused = prepare(
        &scratch.path,
        Some(THIS_RUN),
        Instant::now() + SPENT_DEADLINE,
    )
    .err()
    .map(|error| error.to_string())
    .with_context(|| {
        format!(
            "a lease already held on {} was acquired again, so two processes \
                 would package the same target side by side",
            scratch.path
        )
    })?;
    ensure!(
        refused.contains(LEASE_FILE_NAME),
        "the refusal did not name the lease it waited on: {refused}"
    );
    drop(held);
    Ok(())
}

/// Clearing the record makes the next caller package again.
///
/// This is what runs before a packaging step, so a run that replaces the roots
/// cannot leave a record describing the ones it replaced.
#[rstest]
fn a_cleared_record_is_not_reused(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let lease = scratch.take()?;
    let directory = scratch.open()?;
    record_complete(&directory, THIS_RUN)?;
    clear_record(&directory);
    drop(lease);

    let prepared = prepare(
        &scratch.path,
        Some(THIS_RUN),
        Instant::now() + AMPLE_DEADLINE,
    )?;
    let Prepared::Owned { .. } = prepared else {
        anyhow::bail!("a cleared record was still reused");
    };
    Ok(())
}

/// The suite packages once per run, not once per contract.
///
/// # Why this reads the source
///
/// The property is "how many times does one run enter the packaging boundary".
/// A runtime observation of one contract cannot see a second call site in
/// another module, and the duplication this parameterises was exactly that: six
/// modules each calling `prepare_packaged_set`, invisible to each other, each
/// emptying a process-local cache in its own process.
///
/// So the shape of the code is what is pinned.
///
/// - only `roots` may run a packaging invocation;
/// - the packaged set is prepared at most once per contract, and the reuse is
///   what carries it to the rest;
/// - the lease is what makes that true across processes.
///
/// A fifth call site added to a contract module is not a failure by itself -
/// the lease still deduplicates it - but a call site that bypasses
/// `prepare_packaged_set` and runs packaging directly would restore the
/// duplication. That is what this refuses.
#[rstest]
fn the_packaging_boundary_is_entered_once_per_run() -> Result<()> {
    let sources = repository_root()?.join("ortho_l10n/tests");
    let packaged: usize = count_in_tree(&sources.join("boundary_tests"), "prepare_packaged_set")?;
    let invocations = PACKAGING_INVOCATIONS.len();

    // The packaging step runs from `roots` alone. Matched as a call rather than
    // a bare name, so the declaration and its import do not count as
    // invocations. The needle is split so this test's own source does not
    // contain it and cannot count itself.
    let packaging = count_in_tree(
        &sources.join("boundary/package_set"),
        concat!("package_sources", "(repository"),
    )?;
    ensure!(
        packaging == 1,
        "the packaging boundary is invoked from {packaging} places under \
         tests/boundary/package_set; it must be the single call in roots, or a \
         contract can package without the lease"
    );
    ensure!(
        invocations > 0 && invocations <= 2,
        "the suite performs {invocations} packaging invocations per run; the lease \
         makes that pair the whole run's packaging cost, and a third invocation \
         would not fit the per-test allowance"
    );
    ensure!(
        packaged > 0,
        "no contract calls prepare_packaged_set, so the packaging boundary is dead \
         and this contract proves nothing"
    );
    Ok(())
}

/// How many times `needle` appears in the `.rs` files under `root`.
///
/// Counts occurrences rather than files, because two call sites in one module
/// are exactly the duplication this looks for.
fn count_in_tree(root: &Utf8Path, needle: &str) -> Result<usize> {
    let root_directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())
        .with_context(|| format!("open {root}"))?;
    let mut total = 0;
    let mut pending = vec![(root_directory, String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        for listed in directory
            .entries()
            .with_context(|| format!("read {root}/{prefix}"))?
        {
            let entry = listed.with_context(|| format!("read an entry under {root}/{prefix}"))?;
            let name = entry
                .file_name()
                .with_context(|| format!("read a file name under {root}/{prefix}"))?;
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry
                .file_type()
                .with_context(|| format!("read the type of {root}/{relative}"))?;
            if file_type.is_dir() {
                pending.push((entry.open_dir().context("open a subdirectory")?, relative));
                continue;
            }
            let is_rust = Utf8Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"));
            if !file_type.is_file() || !is_rust {
                continue;
            }
            // Relative to the directory the entry came from, not to the root:
            // `directory` is the parent, so only the final name is a path in it.
            let text = directory
                .read_to_string(&name)
                .with_context(|| format!("read {root}/{relative}"))?;
            total += text.matches(needle).count();
        }
    }
    Ok(total)
}

/// The packaged set the lease protects is the one the contracts read.
///
/// A lease over a directory nothing reads would be a lock around nothing. This
/// ties the two together by name, so renaming one without the other fails here.
#[rstest]
fn the_lease_protects_the_set_the_contracts_read() -> Result<()> {
    ensure!(
        !PACKAGED_CRATES.is_empty(),
        "the packaged set is empty, so the lease protects nothing"
    );
    let roots = repository_root()?.join("ortho_l10n/tests/boundary/package_set/roots.rs");
    let directory = Dir::open_ambient_dir(
        roots
            .parent()
            .context("the packaged roots module has no parent")?,
        cap_std::ambient_authority(),
    )
    .with_context(|| format!("open the directory holding {roots}"))?;
    let source = directory
        .read_to_string("roots.rs")
        .with_context(|| format!("read {roots}"))?;
    ensure!(
        source.contains("prepare(&target"),
        "the packaged set is no longer built through the lease; the twelve \
         invocations a run used to make are back"
    );
    Ok(())
}

/// The lease's poll interval is short enough to be a hand-off, not a stall.
///
/// A blocked contract wakes this often to re-check the record. A value near the
/// packaging cost would add most of an interval to every waiting contract; a
/// value near zero would spin. The bound is stated so neither drifts in
/// unnoticed.
#[rstest]
fn the_lease_poll_interval_stays_a_handoff() -> Result<()> {
    ensure!(
        LEASE_POLL_INTERVAL <= Duration::from_millis(500),
        "the lease poll interval is {LEASE_POLL_INTERVAL:?}; a blocked contract \
         would add that much latency after the packaging step completes"
    );
    ensure!(
        !LEASE_POLL_INTERVAL.is_zero(),
        "a zero poll interval would spin while another process packages"
    );
    Ok(())
}
