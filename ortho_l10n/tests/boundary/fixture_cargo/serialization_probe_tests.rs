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

use super::super::budget::Budget;
use super::probe_support::{DESCENDANT_STARTUP, probe_command_with, scratch_directory, wait_until};
use super::run_with_deadline;
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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

    let first_run = std::thread::spawn(move || -> Result<Output> {
        let bound = Budget::new().bound(FIRST_DEADLINE, "first serialization probe")?;
        run_with_deadline(first, bound, "first serialization probe")
    });
    // Start the second run only once the first child is running, so a missing
    // lock would let the two overlap inside the observation window rather than
    // depending on which thread happened to reach the runner first.
    wait_until(DESCENDANT_STARTUP, "the first probe to start", || {
        Ok(directory.exists("first.start"))
    })?;
    let second_output = run_with_deadline(
        second,
        Budget::new().bound(SECOND_DEADLINE, "second serialization probe")?,
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
