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
//! the checkout over rather than deadlocking it.
//!
//! A third case pins the wiring rather than the lock, because the two above
//! pass whether or not anything in the suite ever calls [`hold_run`]. That is
//! the failure that matters: the guard is only worth its lock if the production
//! path takes it, and takes it *before* reaching the packaging step, so a
//! dropped or reordered acquisition is noticed here rather than by a race
//! nothing observes.
//!
//! The two lock cases take the guard in this process on a scratch directory,
//! for the same reason [`super::super::lease::contention_tests`] holds the
//! packaging lease there: the lock is per open file description on Unix and per
//! sharing mode on Windows, so a second open from this same process contends
//! exactly as a second process would.

use crate::boundary::fixture_cargo::repository_root;
use crate::boundary::package_set::run_guard::{RunGuard, hold_run};
use crate::boundary::package_set::scratch::{Scratch, scratch_root};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
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

/// The production packaging path takes the guard, before it packages.
///
/// [`hold_run`] holding a lock correctly proves nothing about whether anything
/// calls it. Delete the call from `build_packaged_set` and every runtime case
/// in this module still passes, because they call [`hold_run`] themselves; the
/// defect that returns is the cross-process race the guard exists to close, and
/// nothing in the suite would notice it.
///
/// Three shape facts, each closing a different way the wiring could be lost:
///
/// - `roots.rs` calls `hold_run(&boundary` exactly once, so the acquisition
///   cannot be dropped or duplicated;
/// - that call appears *before* the leased `prepare(&target` call, so a lock
///   taken after packaging would not cover the replace it is meant to prevent;
/// - the result is stored in `RUN_GUARD`, not bound to a local. A local would
///   drop at the end of the call, releasing the checkout while the contracts
///   that were promised the roots are still reading them.
///
/// Source-reading rather than behavioural because the behaviour it guards is a
/// lock held for a *process*: a case that observed it would have to reach
/// `build_packaged_set`, which packages for real.
#[rstest]
fn the_packaging_path_takes_the_run_guard_before_packaging() -> Result<()> {
    let roots = repository_root()?.join("ortho_l10n/tests/boundary/package_set/roots.rs");
    let source = read_roots(&roots)?;

    let acquisitions = source.matches(concat!("hold_run", "(&boundary")).count();
    ensure!(
        acquisitions == 1,
        "roots.rs acquires the run guard {acquisitions} times; it must be exactly \
         once, or the checkout is either left unguarded or locked twice over"
    );

    let acquired_at = source
        .find(concat!("hold_run", "(&boundary"))
        .context("the guard acquisition was counted but not located")?;
    let packaged_at = source
        .find(concat!("prepare", "(&target"))
        .context("roots.rs no longer reaches the leased prepare(&target, ..) call")?;
    ensure!(
        acquired_at < packaged_at,
        "roots.rs reaches the packaging step before taking the run guard; a lock \
         taken afterwards does not cover the replace it exists to prevent"
    );

    ensure!(
        source.contains(concat!("RUN_GUARD", ".get_or_init")),
        "roots.rs does not park the run guard in the RUN_GUARD static; a guard \
         bound to a local would drop at the end of the call and release the \
         checkout while the contracts reading the roots are still running"
    );
    Ok(())
}

/// Read `roots.rs` as text.
///
/// Its directory rather than its path, so the read goes through a capability
/// handle like every other source read in these modules.
fn read_roots(roots: &Utf8Path) -> Result<String> {
    let directory = Dir::open_ambient_dir(
        roots
            .parent()
            .context("the packaged roots module has no parent")?,
        cap_std::ambient_authority(),
    )
    .with_context(|| format!("open the directory containing {roots}"))?;
    directory
        .read_to_string("roots.rs")
        .with_context(|| format!("read {roots}"))
}
