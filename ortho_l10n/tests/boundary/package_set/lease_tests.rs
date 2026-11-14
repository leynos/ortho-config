//! Regressions for the packaging lease and the run-scoped completion record.
//!
//! The lease exists so that one nextest *run* performs one packaging pair, not
//! one per test process. Four separate properties have to hold for that to be
//! true, and each has its own failure mode, so each has its own case:
//!
//! - a record written for this run makes the next caller reuse the roots;
//! - a record written for a *different* run does not, because the roots may be
//!   stale;
//! - a lease held elsewhere is reported as a wait, never as a silent second
//!   packaging step;
//! - a waiter that wins the lease *after* the holder recorded reuses the roots,
//!   which is the re-check under the lock rather than the arrival check.
//!
//! The string relation behind the first two is stated as a `proptest` over
//! arbitrary scopes and record contents, because the near-misses - a prefix, a
//! suffix, a superstring, a case variant, the empty string, no scope at all -
//! are exactly where a sloppy comparison hides. The `#[rstest]` cases stay as
//! the readable contract examples.
//!
//! One case is about failure rather than reuse: a record that exists and cannot
//! be read must be *reported*, not read as "not complete", because the latter
//! would silently package beside a contract that may be about to succeed.
//!
//! Every case here starts at an edge of the lease's life: no holder, a spent
//! deadline, a record already present. [What a waiter sees while a holder is
//! packaging](super::lease::contention_tests) covers the interior, where a
//! contract is genuinely waiting behind another and observes the release or the
//! record as it happens. `a_free_lease_is_taken_within_the_poll_bound` sits
//! between the two: it holds nothing, but it bounds what an uncontended
//! acquisition costs, which is the half of the wait's cost no contender can
//! show.
//!
//! The structural cases live in [`super::lease_shape_tests`]; they read sources
//! and constants rather than driving the lease.

use crate::boundary::package_set::lease::{LEASE_FILE_NAME, Prepared, claim, prepare};
use crate::boundary::package_set::record::{
    COMPLETION_FILE_NAME, clear_record, read_unlocked, record_complete,
};
use crate::boundary::package_set::scratch::{AMPLE_DEADLINE, Scratch, WAKE_UP_BOUND, scratch_root};
use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use rstest::rstest;
use std::time::{Duration, Instant};

/// A deadline already in the past, so a held lease is reported at once.
const SPENT_DEADLINE: Duration = Duration::ZERO;

/// The run identifier these cases record under.
///
/// Not `NEXTEST_RUN_ID`: a real run's value would be visible to the other
/// contracts in the same binary and could make one of them reuse roots this
/// case's scratch directory does not hold.
const THIS_RUN: &str = "lease-regression-this-run";

/// A different run identifier, standing for an earlier run's record.
const OTHER_RUN: &str = "lease-regression-another-run";

/// A record for this run makes the roots reusable without packaging.
#[rstest]
fn a_record_for_this_run_is_reused(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let lease = scratch.take(Some(THIS_RUN))?;
    record_complete(&lease, THIS_RUN)?;
    drop(lease);

    let prepared = prepare(
        scratch.path(),
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
    let lease = scratch.take(Some(THIS_RUN))?;
    record_complete(&lease, OTHER_RUN)?;
    drop(lease);

    let prepared = prepare(
        scratch.path(),
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
    let lease = scratch.take(None)?;
    record_complete(&lease, THIS_RUN)?;
    drop(lease);

    let prepared = prepare(scratch.path(), None, Instant::now() + AMPLE_DEADLINE)?;
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
    let held = scratch.take(Some(THIS_RUN))?;

    let refused = prepare(
        scratch.path(),
        Some(THIS_RUN),
        Instant::now() + SPENT_DEADLINE,
    )
    .err()
    .map(|error| error.to_string())
    .with_context(|| {
        format!(
            "a lease already held on {} was acquired again, so two processes \
                 would package the same target side by side",
            scratch.path()
        )
    })?;
    ensure!(
        refused.contains(LEASE_FILE_NAME),
        "the refusal did not name the lease it waited on: {refused}"
    );
    drop(held);
    Ok(())
}

/// A waiter that wins the lease after the holder records reuses the roots.
///
/// This is the cross-process saving, and it is a property of the *claim*, not
/// of the arrival check: a contract whose first look found nothing still must
/// not package once the lock it waited for is its own. Without the re-check
/// under the lease, that contract owns a step somebody else already finished.
///
/// Driven through [`claim`] rather than [`prepare`] because that is where the
/// re-check lives, and because the window it covers - the record appearing
/// between the arrival check and the winning lock attempt - cannot be hit
/// deterministically from outside. [`prepare`] is not a substitute: with the
/// record already present it returns `Reused` at the arrival check and never
/// reaches the claim, so a `prepare`-driven case passes whether or not the
/// re-check exists. Calling `claim` with the lease free and the record present
/// is exactly the state a waiter reaches when it wins that race, and it
/// distinguishes the two implementations: with the re-check the answer is
/// `Reused`, and without it the answer is `Owned`.
#[rstest]
fn a_waiter_that_wins_the_lease_after_the_record_reuses_the_roots(
    scratch_root: Result<Scratch>,
) -> Result<()> {
    let scratch = scratch_root?;
    let holder = scratch.take(Some(THIS_RUN))?;
    record_complete(&holder, THIS_RUN)?;
    drop(holder);

    let claimed = claim(&scratch.open()?, Some(THIS_RUN), scratch.path())?;
    ensure!(
        matches!(claimed, Some(Prepared::Reused)),
        "a contract that won the lease after the holder recorded was told to \
         package again, so a waiter repeats work the holder already finished"
    );
    Ok(())
}

/// A free lease is taken inside the poll bound.
///
/// No lease is held here and no record is present, so this is the uncontended
/// opening of the step rather than a hand-off: it bounds what the acquisition
/// itself costs when nothing stands in the way. A `prepare` that waited on the
/// poll interval before its first attempt, or that slept for the deadline
/// rather than for an interval, fails this and nothing else. The contended
/// hand-off it might be mistaken for is
/// [`super::contention_tests::a_waiter_behind_a_holder_takes_the_step_once_it_is_released`],
/// which holds the lease and observes the waiter.
#[rstest]
fn a_free_lease_is_taken_within_the_poll_bound(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;

    let started = Instant::now();
    let prepared = prepare(
        scratch.path(),
        Some(THIS_RUN),
        Instant::now() + AMPLE_DEADLINE,
    )?;
    let Prepared::Owned { lease } = prepared else {
        anyhow::bail!("an unheld target with no record was reported as already packaged");
    };
    let waited = started.elapsed();
    ensure!(
        waited <= WAKE_UP_BOUND,
        "taking a free lease took {waited:?}, past the {WAKE_UP_BOUND:?} a waiter \
         may add after a release; acquisition is sleeping before it tries"
    );
    drop(lease);
    Ok(())
}

/// Clearing the record makes the next caller package again.
///
/// This is what runs before a packaging step, so a run that replaces the roots
/// cannot leave a record describing the ones it replaced.
#[rstest]
fn a_cleared_record_is_not_reused(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let lease = scratch.take(Some(THIS_RUN))?;
    record_complete(&lease, THIS_RUN)?;
    clear_record(&lease)?;
    drop(lease);

    let prepared = prepare(
        scratch.path(),
        Some(THIS_RUN),
        Instant::now() + AMPLE_DEADLINE,
    )?;
    let Prepared::Owned { .. } = prepared else {
        anyhow::bail!("a cleared record was still reused");
    };
    Ok(())
}

/// A record that cannot be read is an error, not "not complete".
///
/// The distinction is the whole of the fallible read. Reporting a permission or
/// handle fault as "nothing recorded yet" would let this contract start
/// packaging beside a contract that may be about to succeed, and the resulting
/// double pack would look like a slow run rather than a fault. A record file
/// that is a *directory* is the portable way to make the read fail with
/// something other than `NotFound`: every platform refuses it, and none of them
/// reports it as absence.
#[rstest]
fn an_unreadable_record_is_reported_rather_than_treated_as_absent(
    scratch_root: Result<Scratch>,
) -> Result<()> {
    let scratch = scratch_root?;
    let directory = scratch.open()?;
    directory
        .create_dir(COMPLETION_FILE_NAME)
        .context("make the completion record unreadable by replacing it with a directory")?;

    let read = read_unlocked(&directory, Some(THIS_RUN));
    let error = read
        .err()
        .map(|error| format!("{error:#}"))
        .with_context(|| {
            format!(
                "an unreadable record in {} read as \"not complete\", so this \
             contract would package beside a contract that may be about to finish",
                scratch.path()
            )
        })?;
    ensure!(
        error.contains("completion record"),
        "the read failure was reported without naming the record: {error}"
    );
    Ok(())
}

/// Turn a scratch-directory failure into a property failure.
///
/// `proptest!` threads `TestCaseError` through the body, not `anyhow::Error`,
/// so the fixture's errors are converted here. A failed allocation is then
/// reported as the property's failure with its context intact, rather than
/// panicking the runner and losing which case provoked it.
fn case_failure(error: &anyhow::Error) -> TestCaseError {
    TestCaseError::fail(format!("{error:#}"))
}

proptest! {
    /// A record is reused exactly when a scope was given and the bytes match.
    ///
    /// This is the rule `prepare` applies at both of its check points, stated
    /// over the whole string space rather than over chosen examples. The
    /// strategy draws the scope, the record's bytes, and whether a record exists
    /// at all, so "nothing recorded yet" is covered by the same law. Exact
    /// equality is the assertion: a `starts_with`, `contains` or case-folding
    /// comparison would let a prefix, a suffix or a case variant pass, and each
    /// of those names a *different* run's roots.
    #[test]
    fn the_record_is_reused_exactly_when_it_names_a_given_scope(
        scope in prop::option::of(".{0,40}"),
        recorded in ".{0,40}",
        record_exists in any::<bool>(),
    ) {
        let scratch = scratch_root().map_err(|error| case_failure(&error))?;
        let directory = scratch.open().map_err(|error| case_failure(&error))?;
        if record_exists {
            let holder = scratch.take(Some(THIS_RUN)).map_err(|error| case_failure(&error))?;
            record_complete(&holder, &recorded).map_err(|error| case_failure(&error))?;
            drop(holder);
        }

        let reused = read_unlocked(&directory, scope.as_deref()).map_err(|error| case_failure(&error))?;
        let expected = record_exists && scope.as_deref() == Some(recorded.as_str());
        prop_assert_eq!(
            reused,
            expected,
            "a record holding {:?} (present: {}) read as {} under scope {:?}",
            recorded,
            record_exists,
            reused,
            scope,
        );
    }
}

/// The record's name is a dot-file beside the archives, not among them.
///
/// A record whose name collided with a packaged artefact would be overwritten
/// by the next `cargo package`, and the run would re-package for a reason that
/// looked like a lost cache entry. Cheap to state, so it is stated.
#[rstest]
fn the_record_name_is_not_a_packaged_artefact() {
    assert!(
        COMPLETION_FILE_NAME.starts_with('.'),
        "the completion record is named {COMPLETION_FILE_NAME:?}, which `cargo \
         package` may write over"
    );
}
