//! Regression for the in-process serialization of nested Cargo runs.
//!
//! `cargo test` runs a binary's tests as threads in one process, so two
//! contracts can enter [`super::run_with_deadline`] at the same time and race
//! for the same Cargo locks. [`super::NESTED_CARGO`] serializes them, and the
//! lock is taken *before* the child is spawned so a sibling's wait is never
//! charged to the child's deadline.
//!
//! The regression starts one probe through the runner on its own thread, waits
//! until that child is demonstrably running, then runs a second probe whose own
//! deadline is shorter than the first probe's lifetime. Both must succeed: the
//! second child may only start after the first has exited, and it must then get
//! its full deadline rather than inherit the time spent waiting for the lock.
//! Without the lock the two children overlap and the ordering assertion fails;
//! with the lock taken after the deadline was computed, the second probe is
//! killed and the call returns an error. Both failures are distinguished from
//! the successful case by a wide margin, so the regression does not race the
//! scheduler.
//!
//! Two further regressions hold the serialization themselves, so the wait they
//! measure lasts exactly as long as they choose rather than depending on a
//! sibling contract's build:
//!
//! - [`an_allowance_spent_while_waiting_is_reported`] gives the waiting call an
//!   allowance shorter than the hold. The call must refuse at the allowance,
//!   name the wait, and never spawn its child.
//! - [`a_wait_for_a_sibling_is_charged_to_the_allowance`] holds the lock for
//!   part of a longer allowance, then releases it. The child's bound must be
//!   what is left, not the allowance the call started with: a bound computed
//!   before the wait would let the child finish a sleep longer than the
//!   remainder.

use super::super::budget::Budget;
use super::probe_support::{probe_command_with, scratch_directory, wait_until};
use super::{NESTED_CARGO, POLL_INTERVAL, run_with_deadline};
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, MutexGuard, TryLockError, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

/// Names the probe variant the child should run.
const PROBE_ID_VARIABLE: &str = "BOUNDARY_SERIALIZATION_PROBE_ID";

/// How long the probe holds its run before exiting, in milliseconds.
const PROBE_SLEEP_VARIABLE: &str = "BOUNDARY_SERIALIZATION_PROBE_SLEEP_MILLIS";

/// How long the first probe runs.
///
/// Comfortably longer than the second probe's whole deadline, so the two cases
/// are separated by seconds rather than by scheduling noise: an unserialized
/// second child starts at once, while a serialized one cannot start until this
/// sleep has ended.
const FIRST_SLEEP: Duration = Duration::from_secs(4);

/// How long the second probe runs once it is allowed to start.
const SECOND_SLEEP: Duration = Duration::from_secs(2);

/// The first probe's deadline, which must exceed its own sleep.
const FIRST_DEADLINE: Duration = Duration::from_secs(10);

/// The second probe's deadline, shorter than the first probe's sleep.
///
/// A run that charged the lock wait to this deadline would kill the second
/// child before it could do any work; a run that starts the clock after the
/// lock is granted lets the child finish its own sleep and exit normally.
const SECOND_DEADLINE: Duration = Duration::from_secs(3);

/// How long the first probe may take to report that it has started.
///
/// The probe queues behind the process-wide serialization every nested Cargo
/// call in this binary shares, and a sibling contract's `cargo package` may
/// hold that for its own 440-second bound. A bound sized only for process
/// startup would report a sibling's legitimate build as this regression's
/// failure. The first run's own completion is watched as well, so a real
/// failure still reports at once instead of waiting this out.
const FIRST_START_WAIT: Duration = Duration::from_secs(600);

/// How long a regression waits to take the serialization for itself.
///
/// A sibling contract may legitimately hold it for its own 440-second cap.
/// Waiting for that is the point of the regression below, not a failure, so
/// this bound only stops a wedged binary from hanging the suite.
const SERIALIZATION_ACQUIRE: Duration = Duration::from_secs(600);

/// The allowance given to the call whose wait is the thing under test.
///
/// Shorter than the hold below, so the allowance expires during the wait rather
/// than after it.
const WAIT_ALLOWANCE: Duration = Duration::from_secs(1);

/// The cap of the waiting call. It must not be what stops the call: the
/// allowance has to be exhausted first, which is what the regression checks.
const WAIT_CAP: Duration = Duration::from_secs(30);

/// How long the regression waits for the refused call to return.
const REFUSAL_LIMIT: Duration = Duration::from_secs(30);

/// The allowance shared between the hold and the child it pays for.
const SHARED_ALLOWANCE: Duration = Duration::from_secs(12);

/// How long the second regression holds the serialization.
///
/// Long enough that the remaining allowance is shorter than the child's own
/// sleep, and short enough that it leaves a positive remainder to spawn with.
const SHARED_HOLD: Duration = Duration::from_secs(5);

/// The child's own sleep, longer than the allowance that remains after the hold.
const SHARED_SLEEP: Duration = Duration::from_secs(9);

/// The cap of the charged call, far longer than the allowance, so a bound taken
/// before the wait would have let the child finish its sleep.
const SHARED_CAP: Duration = Duration::from_secs(60);

/// How long the charged call may take once it owns the serialization.
const CHARGED_LIMIT: Duration = Duration::from_secs(60);

/// Write a marker and sleep, then write the exit marker.
///
/// Ignored so an ordinary run never waits for it; the regression selects it by
/// name and supplies the variant and sleep through the environment. The marker
/// contents are wall-clock milliseconds, because the parent compares them
/// across processes where `Instant` is not comparable.
#[test]
#[ignore = "driven as a child process by concurrent_runs_are_serialized"]
fn serialization_probe() -> Result<()> {
    let id = std::env::var(PROBE_ID_VARIABLE)
        .with_context(|| format!("{PROBE_ID_VARIABLE} should name the probe variant"))?;
    let sleep_millis = std::env::var(PROBE_SLEEP_VARIABLE)
        .with_context(|| format!("{PROBE_SLEEP_VARIABLE} should hold the sleep"))?
        .parse::<u64>()
        .context("parse the probe's sleep")?;
    let scratch = scratch_directory()?;
    write_marker(&scratch, &format!("{id}.start"))?;
    std::thread::sleep(Duration::from_millis(sleep_millis));
    write_marker(&scratch, &format!("{id}.exit"))?;
    Ok(())
}

/// Two nested runs must not overlap, and the second must keep its own deadline.
#[test]
fn concurrent_runs_are_serialized() -> Result<()> {
    let scratch = TempDir::new().context("create the serialization scratch directory")?;
    let directory = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the serialization scratch directory")?;
    let first = probe_command("first", FIRST_SLEEP, scratch.path())?;
    let second = probe_command("second", SECOND_SLEEP, scratch.path())?;

    let first_finished = Arc::new(AtomicBool::new(false));
    let finished_flag = Arc::clone(&first_finished);
    let first_run = std::thread::spawn(move || -> Result<Output> {
        let outcome = run_with_deadline(
            first,
            Budget::new(),
            FIRST_DEADLINE,
            "first serialization probe",
        );
        // Set on every path, so the parent never waits out the bound for a run
        // that has already returned.
        finished_flag.store(true, Ordering::SeqCst);
        outcome
    });
    // Start the second run only once the first child is running, so a missing
    // lock would let the two overlap inside the observation window rather than
    // depending on which thread happened to reach the runner first.
    wait_until(FIRST_START_WAIT, "the first probe to start", || {
        Ok(directory.exists("first.start") || first_finished.load(Ordering::SeqCst))
    })?;
    ensure!(
        directory.exists("first.start"),
        "the first serialization probe returned without reporting that it started",
    );
    let second_output = run_with_deadline(
        second,
        Budget::new(),
        SECOND_DEADLINE,
        "second serialization probe",
    )
    .context(
        "the second serialization probe should start after the first finishes \
         and keep its own full deadline",
    )?;
    let first_output = first_run
        .join()
        .map_err(|_| anyhow!("the first serialization probe's thread panicked"))??;

    ensure!(
        first_output.status.success() && second_output.status.success(),
        "both probes should exit successfully: first {:?}, second {:?}",
        first_output.status,
        second_output.status,
    );
    let first_exit = read_marker(&directory, "first.exit")?;
    let second_start = read_marker(&directory, "second.start")?;
    let second_exit = read_marker(&directory, "second.exit")?;
    ensure!(
        second_start >= first_exit,
        "the second probe started {second_start} ms and the first exited {first_exit} ms; \
         nested runs overlapped instead of being serialized",
    );
    ensure!(
        second_exit.saturating_sub(second_start) >= SECOND_SLEEP.as_millis(),
        "the second probe ran for {} ms rather than its own {} ms sleep, so its \
         deadline covered time spent waiting for the lock",
        second_exit.saturating_sub(second_start),
        SECOND_SLEEP.as_millis(),
    );
    Ok(())
}

/// A wait that outlives the allowance must be reported, not hidden.
///
/// The regression takes the serialization itself, so the call below cannot
/// acquire it, and gives that call an allowance far shorter than the hold. The
/// call must return at its allowance while the lock is still held: a runner
/// that waited without a bound would block here until the test dropped the
/// lock, and a runner that ignored the wait when bounding the child would spawn
/// one after the allowance had already been spent.
#[test]
fn an_allowance_spent_while_waiting_is_reported() -> Result<()> {
    let scratch = TempDir::new().context("create the wait-allowance scratch directory")?;
    let directory = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the wait-allowance scratch directory")?;
    let held = hold_serialization()?;
    // Created before the thread starts, so the allowance is spent from a known
    // instant rather than from whenever the thread is scheduled.
    let budget = Budget::expiring_in(WAIT_ALLOWANCE);
    let command = probe_command("waiter", Duration::ZERO, scratch.path())?;
    let (returned, outcome) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let result =
            run_with_deadline(command, budget, WAIT_CAP, "the waiting serialization probe");
        let _sent = returned.send(());
        result
    });

    let reported = outcome
        .recv_timeout(REFUSAL_LIMIT)
        .context("the runner should refuse at the allowance while it waits for the serialization");
    // The lock is released only after the refusal is observed, so the call
    // cannot have acquired it and spawned a child before returning.
    drop(held);
    reported?;
    let error = waiter
        .join()
        .map_err(|_panic| anyhow!("the waiting probe's thread panicked"))?
        .err()
        .context("a call that spent its allowance waiting should fail")?;
    ensure!(
        error
            .to_string()
            .contains("waited for a sibling nested Cargo invocation"),
        "the error should name the wait that spent the allowance:\n{error:#}",
    );
    ensure!(
        !directory.exists("waiter.start"),
        "the runner spawned a child after the allowance had been spent waiting",
    );
    Ok(())
}

/// The child's bound must be what is left after the serialization wait.
///
/// The regression holds the serialization for part of the shared allowance,
/// then releases it. The child sleeps longer than the remainder, so it must be
/// killed at what is left. A bound computed before the wait would have been the
/// whole allowance, which comfortably covers the child's sleep, and the call
/// would succeed instead.
#[test]
fn a_wait_for_a_sibling_is_charged_to_the_allowance() -> Result<()> {
    let scratch = TempDir::new().context("create the charged-wait scratch directory")?;
    let directory = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the charged-wait scratch directory")?;
    let held = hold_serialization()?;
    let budget = Budget::expiring_in(SHARED_ALLOWANCE);
    let command = probe_command("charged", SHARED_SLEEP, scratch.path())?;
    let (finished, outcome) = mpsc::channel();
    let runner = std::thread::spawn(move || {
        let result = run_with_deadline(command, budget, SHARED_CAP, "the allowance-charged probe");
        let _sent = finished.send(());
        result
    });

    // Spend part of the allowance while the child's thread is queued behind the
    // lock, then let it through. Spawning the command above happens inside the
    // thread, so the wait and the child's bound are paid from one allowance.
    std::thread::sleep(SHARED_HOLD);
    drop(held);
    outcome
        .recv_timeout(CHARGED_LIMIT)
        .context("the runner should finish once it owns the serialization")?;
    let error = runner
        .join()
        .map_err(|_panic| anyhow!("the charged probe's thread panicked"))?
        .err()
        .context(
            "the remaining allowance should have bounded the child; a bound taken before the \
             wait would have covered the child's whole sleep",
        )?;
    ensure!(
        error.to_string().contains("allowance"),
        "the error should report the shared allowance rather than a bare cap:\n{error:#}",
    );
    ensure!(
        !directory.exists("charged.exit"),
        "the child finished its sleep, so its bound was not the remaining allowance",
    );
    Ok(())
}

/// Take the in-process serialization for the duration of one regression.
///
/// A sibling contract may hold it legitimately, so acquisition is bounded and
/// reported rather than blocking the whole binary behind a wedged lock.
fn hold_serialization() -> Result<MutexGuard<'static, ()>> {
    let limit = Instant::now() + SERIALIZATION_ACQUIRE;
    loop {
        match NESTED_CARGO.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(poisoned)) => return Ok(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {}
        }
        ensure!(
            Instant::now() < limit,
            "timed out taking the nested-Cargo serialization for a regression",
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Build the probe command for one variant and sleep.
fn probe_command(id: &str, sleep: Duration, scratch: &Path) -> Result<Command> {
    let mut command = probe_command_with("serialization_probe", scratch)?;
    command
        .env(PROBE_ID_VARIABLE, id)
        .env(PROBE_SLEEP_VARIABLE, sleep.as_millis().to_string());
    Ok(command)
}

/// Write the current wall-clock time, in milliseconds, to `name`.
fn write_marker(scratch: &Dir, name: &str) -> Result<()> {
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("read the wall clock for a serialization marker")?
        .as_millis();
    scratch
        .write(name, at.to_string())
        .with_context(|| format!("write the serialization marker {name}"))
}

/// Read one marker's millisecond timestamp.
fn read_marker(scratch: &Dir, name: &str) -> Result<u128> {
    scratch
        .read_to_string(name)
        .with_context(|| format!("read the serialization marker {name}"))?
        .trim()
        .parse::<u128>()
        .with_context(|| format!("parse the serialization marker {name}"))
}
