//! Bounded contention for the artefact-lock probes.
//!
//! Issue #606's failures are intermittent: the probe tests pass in isolation and
//! fail roughly once every few full-binary runs at high `--test-threads`. One
//! green run therefore says almost nothing, so the sequence that failed needs a
//! case that keeps it running under concurrent load for a fixed number of
//! iterations.
//!
//! The stress is bounded by iteration count, never by wall-clock time. A
//! duration-bounded loop would run longer on a loaded machine and so produce a
//! different amount of evidence per run, leaving a failure that could not be
//! reproduced by iteration number.
//!
//! Each worker owns its own [`TempRoot`]. A shared root would make "the lock is
//! free" genuinely false whenever a peer held it, so the suite would report
//! contention as if it were a leak. Distinct roots keep every free assertion
//! sound while still running the probes concurrently, which is the condition the
//! intermittent failures were observed under.
//!
//! Every free assertion routes through `lock_diagnostics`, so a failure names the
//! process and descriptor still holding the lock instead of restating the
//! boolean that could not explain the failure in the first place.
//!
//! Two of those assertions exist, and the split between them is the point. The
//! gating suite settles before failing, because a peer test that forks makes a
//! released lock read as held for the length of that `fork` — `flock` belongs to
//! the open file description, which a child inherits and loses at `exec`. The
//! ignored fork variant asserts strictly instead: it exists to put that window
//! under deliberate pressure and to report whoever is holding the lock when it
//! lands.

use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;

#[cfg(target_os = "linux")]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, ensure};

use super::artefact_lock::{create_lock, is_free, lock};
use super::fragment_tests::TempRoot;
#[cfg(target_os = "linux")]
use super::lock_diagnostics::ensure_released;
use super::lock_diagnostics::ensure_settles_free;

/// How many workers run the probe cycles at once.
///
/// Small and fixed. The failures were observed under a whole test binary at
/// high parallelism, so a handful of concurrent workers reproduces the
/// scheduling pressure without multiplying the runtime.
const WORKERS: usize = 4;

/// How many probe cycles each worker runs.
///
/// Large enough that the cycles overlap across workers for a useful span, and
/// small enough that the suite stays cheap beside the rest of the library run.
const ITERATIONS: usize = 32;

/// How many probe cycles the fork-window variant runs.
///
/// Only a run asked for by name reaches this, so the bound only has to be large
/// enough for a fork to land inside the window between a release and the probe
/// that follows it.
#[cfg(target_os = "linux")]
const FORK_ITERATIONS: usize = 64;

/// Runs one iteration of the two probe shapes issue #606 failed in.
///
/// The order matters: the first two probes repeat the release-after-probe case,
/// and the guard plus drop that follow repeat the release-after-guard case.
/// Every free assertion carries its own sentence about what should have made the
/// lock free, because that sentence leads the diagnostic report when it did not.
///
/// The free assertions settle rather than demanding an immediate answer, for the
/// reason `ensure_settles_free` records: a peer test forking for the duration of
/// this worker's probe makes a released lock read as held, and that is a
/// property of `flock` rather than a defect this suite should fail on. The
/// ignored fork diagnostic is where that window is explored deliberately.
fn probe_cycles(root: &Path) -> Result<()> {
    ensure_settles_free(root, "an unheld lock must be reported free")?;
    ensure_settles_free(
        root,
        concat!(
            "a probe must release the lock it took, so a second probe still finds ",
            "it free",
        ),
    )?;
    let held = lock(root).context("take the lock for the release cycle")?;
    // Without this the assertions above prove nothing: a guard that never took
    // the lock would leave every later probe legitimately free, and the suite
    // would pass while checking nothing.
    ensure!(
        !is_free(root).context("probe the lock this worker holds")?,
        "a lock this worker holds must be reported as held, not free",
    );
    drop(held);
    ensure_settles_free(root, "releasing the guard must make the lock free again")
}

/// Creates one worker's artefact root and lock file.
///
/// Kept separate from [`stress_worker`] so the worker can reach the barrier
/// before it propagates a set-up failure: the barrier is a rendezvous of fixed
/// size, not a place to report errors from.
fn prepare_root(worker: usize) -> Result<TempRoot> {
    let root =
        TempRoot::new().with_context(|| format!("create worker {worker}'s artefact root"))?;
    create_lock(root.path()).with_context(|| format!("create worker {worker}'s lock file"))?;
    Ok(root)
}

/// Runs one worker's fixed number of probe cycles over its own artefact root.
///
/// The barrier is what makes the workers concurrent rather than merely
/// sequential: every one of them reaches the first cycle before any of them
/// starts, so the cycles overlap from the outset instead of hoping the spawn
/// order lines them up.
fn stress_worker(barrier: &Arc<Barrier>, worker: usize) -> Result<()> {
    let prepared = prepare_root(worker);
    // The barrier is sized for `WORKERS` parties and this worker is one of
    // them, so it must arrive even when its own set-up failed. Returning early
    // would leave the peers waiting on a party that never comes, and `join`
    // would wait on those peers: a set-up error would hang instead of report.
    barrier.wait();
    let root = prepared?;
    for iteration in 0..ITERATIONS {
        probe_cycles(root.path())
            .with_context(|| format!("worker {worker}, iteration {iteration}"))?;
    }
    Ok(())
}

/// Joins every worker, returning the first failure once all have finished.
///
/// All workers are joined before the failure is returned, so one worker's
/// failure cannot leave the others running while the test's roots are dropped
/// beneath them. The first failure is kept because it is the one whose report
/// the schedule produced first; later ones are usually its consequences.
fn join_workers(handles: Vec<thread::JoinHandle<Result<()>>>) -> Result<()> {
    let mut first: Option<anyhow::Error> = None;
    for handle in handles {
        let outcome = handle
            .join()
            .map_err(|payload| anyhow::anyhow!("a stress worker panicked: {payload:?}"))
            .flatten();
        first = first.or_else(|| outcome.err());
    }
    first.map_or_else(|| Ok(()), Err)
}

/// Runs the workers to completion and reports the first failure.
fn run_stress() -> Result<()> {
    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut handles = Vec::with_capacity(WORKERS);
    for worker in 0..WORKERS {
        let worker_barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            stress_worker(&worker_barrier, worker)
        }));
    }
    join_workers(handles)
}

/// Verifies the probes keep their post-release answer under concurrent load.
#[test]
fn probes_stay_correct_under_concurrent_load() -> Result<()> {
    run_stress()
}

/// Forks a short-lived command until told to stop.
///
/// This is what widens the window the ignored variant is looking for. A fork
/// duplicates the whole process's descriptor table, not just the calling
/// thread's, so the child inherits any artefact lock held by *another* worker
/// and keeps that lock's open file description alive until its exec closes the
/// close-on-exec descriptor. `flock` belongs to the description rather than to a
/// process, so for that window a lock its owner has already released still
/// reads as held.
#[cfg(target_os = "linux")]
fn spawn_short_lived_commands(stop: &Arc<AtomicBool>) -> Result<()> {
    while !stop.load(Ordering::Relaxed) {
        let status = Command::new("true")
            .status()
            .context("run the short-lived command")?;
        ensure!(status.success(), "the short-lived command must succeed");
    }
    Ok(())
}

/// Runs one cycle that demands the lock be free the instant it is probed.
///
/// This is the strict form, and it is deliberately *not* what the gating suite
/// uses. Its whole purpose is to make the fork window observable: when a peer
/// is forking, a released lock reads as held for the length of that `fork`, and
/// only an assertion with no settling tolerance turns that into the report
/// naming the holder.
#[cfg(target_os = "linux")]
fn strict_cycles(root: &Path) -> Result<()> {
    ensure_released(root, "an unheld lock must be reported free")?;
    ensure_released(
        root,
        concat!(
            "a probe must release the lock it took, so a second probe still finds ",
            "it free",
        ),
    )?;
    // Both of issue #606's observed failures are covered: the probe pair above,
    // and the guard release below. Dropping the guard is the shape the release
    // tests assert on, so it has to be under the same fork pressure.
    let held = lock(root).context("take the lock for the strict release cycle")?;
    drop(held);
    ensure_released(root, "releasing the guard must make the lock free again")
}

/// Runs probe cycles while a peer thread forks short-lived commands.
///
/// The spawner is stopped and joined before the outcome is reported for two
/// reasons: a thread still forking would outlive the root about to be dropped,
/// and the probe failure is the evidence worth surfacing — a shutdown fault
/// must not replace it. The probe outcome still wins over the spawner's own.
#[cfg(target_os = "linux")]
fn run_fork_stress() -> Result<()> {
    let root = TempRoot::new().context("create the artefact root for the fork variant")?;
    create_lock(root.path()).context("create the lock file for the fork variant")?;
    let stop = Arc::new(AtomicBool::new(false));
    let spawner_stop = Arc::clone(&stop);
    let spawner = thread::spawn(move || spawn_short_lived_commands(&spawner_stop));

    let outcome = (0..FORK_ITERATIONS).try_for_each(|iteration| {
        strict_cycles(root.path()).with_context(|| format!("fork variant, iteration {iteration}"))
    });

    stop.store(true, Ordering::Relaxed);
    let joined = spawner
        .join()
        .map_err(|payload| anyhow::anyhow!("the command spawner panicked: {payload:?}"))
        .flatten();
    outcome?;
    joined
}

/// Reproduces the fork-to-exec window as a diagnostic rather than as a gate.
///
/// Ignored because it is slow and its outcome depends on where the scheduler
/// lands a fork relative to a release. A failure here is *evidence*: it carries
/// the full `ensure_released` report naming the process and descriptor that
/// still held the lock, which is what issue #606 asks for. It is not by itself
/// proof of a leaked handle, and it is why this case must never gate a merge.
#[cfg(target_os = "linux")]
#[test]
// The escaped newline is deliberate here. `#[ignore = concat!(..)]` compiles,
// but libtest only reports a reason written as one string literal: the macro
// form silently degrades the output to a bare "ignored", and that sentence is
// the whole explanation of why this case must never gate a merge.
#[ignore = "widens the fork-to-exec window; slow and schedule-dependent, and a \
            failure here is diagnostic evidence rather than a gate"]
fn probes_survive_a_concurrent_fork() -> Result<()> {
    run_fork_stress()
}
