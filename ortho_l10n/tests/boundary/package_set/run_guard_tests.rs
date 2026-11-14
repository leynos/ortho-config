//! The run guard, which is what makes a `Reused` answer safe to act on.
//!
//! The packaging lease answers "is a packaging step running right now". It does
//! not answer "may the roots I was just promised be replaced while I read
//! them", because it is released before the consumer reads anything. The run
//! guard answers the second question, and its whole content is that a second
//! *process* cannot get past it while a first is alive. That is narrower than
//! excluding two whole runs, which could still interleave process by process;
//! see the module documentation for why the process span is the one that
//! matters here.
//!
//! These cases pin the two halves that a broken guard would lose: a second
//! holder is refused while the first is alive, and the first's release hands
//! the checkout over rather than deadlocking it. The positive behaviour - that
//! every process reaching the target holds one - is a property of the wiring
//! rather than of this module, and is pinned in
//! [`super::super::lease::lease_shape_tests`].
//!
//! Both cases take the guard in this process on a scratch directory, for the
//! same reason [`super::super::lease::contention_tests`] holds the packaging
//! lease there: the lock is per open file description on Unix and per sharing
//! mode on Windows, so a second open from this same process contends exactly as
//! a second process would.

use crate::boundary::package_set::run_guard::{RunGuard, hold_run};
use crate::boundary::package_set::scratch::{Scratch, scratch_root};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::time::{Duration, Instant};

/// A deadline already in the past, so a refused guard reports at once.
const SPENT_DEADLINE: Duration = Duration::ZERO;

/// Far enough away that a guard which should be granted always is.
const AMPLE: Duration = Duration::from_secs(60);

/// A second holder is refused while the first guard lives.
///
/// This is the exclusion the `Reused` answer depends on. If a second call were
/// granted, two runs could be reading and replacing the same roots at once, and
/// every guarantee the record makes about "this run's" packaged set would be a
/// statement about a directory somebody else may be rewriting.
///
/// The refusal is bounded by an already-spent deadline, so the case fails
/// rather than hangs if the guard ever stops excluding: [`hold_run`] polls
/// until its deadline, and this one has none left.
#[rstest]
fn a_second_run_guard_is_refused_while_the_first_is_held(
    scratch_root: Result<Scratch>,
) -> Result<()> {
    let scratch = scratch_root?;
    let _first = hold_run(scratch.path(), Instant::now() + AMPLE)?;

    let refused = hold_run(scratch.path(), Instant::now() + SPENT_DEADLINE)
        .err()
        .map(|error| format!("{error:#}"))
        .with_context(|| {
            format!(
                "a second boundary process was granted the guard on {}; \
                 overlapping processes would replace the packaged roots one \
                 another is reading",
                scratch.path()
            )
        })?;
    ensure!(
        refused.contains("boundary process holds"),
        "the refusal did not say another process holds the guard: {refused}"
    );
    Ok(())
}

/// Releasing the guard hands the checkout to the next run.
///
/// The other half of exclusion: a guard that never let go would not make the
/// suite exclusive, it would make it unrunnable after the first process. This
/// takes one, drops it, and requires the next one to be granted at once.
#[rstest]
fn a_released_run_guard_is_granted_to_the_next_run(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let first = hold_run(scratch.path(), Instant::now() + AMPLE)?;
    drop(first);

    let second: RunGuard = hold_run(scratch.path(), Instant::now() + SPENT_DEADLINE).context(
        "a released run guard was not granted to the next run, so only \
                  the first process in a checkout could ever reach the target",
    )?;
    drop(second);
    Ok(())
}
