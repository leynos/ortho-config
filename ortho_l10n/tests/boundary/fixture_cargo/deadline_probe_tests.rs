//! Regression tests for the deadline runner in [`super::run_with_deadline`].
//!
//! The defects these guard against were silence and orphaned work. Six packaged
//! boundary contracts reported only "exceeded the 240 second deadline" while
//! the nested Cargo child had printed its compile-admission queue in front of
//! it, so the retained evidence could not distinguish a queue wait from a hang.
//! The runner also killed only the direct child and then joined its readers
//! without a bound, so a descendant could keep compiling after a timeout or
//! keep a pipe open after its parent exited.
//!
//! Each regression drives the child through [`super::spawn_owned`] and waits for
//! its own readiness marker before calling [`super::supervise`], so the
//! deadline measures the runner rather than the host's scheduling delay when a
//! hundred test binaries start at once.
//!
//! Asserting any of that needs a real timed-out child and a real descendant.
//! The only portable child this binary can rely on is itself: the ignored
//! probes below re-run this test binary as children, and the descendant probes
//! do the same one level deeper. A descendant's lifetime is observable from the
//! parent through an exclusive file lock: while the descendant lives, the lock
//! cannot be taken, and the kernel releases it when the process dies. The
//! shared scaffolding for all of that lives in [`super::probe_support`].
//!
//! The three failure shapes are covered deliberately. A timeout must kill the
//! tree; a normal parent exit must not be blocked by a descendant holding the
//! pipe; and a timeout must still return when a descendant escaped the kill
//! entirely, because that is the one state in which the pipe never closes.

use super::probe_support::descendant_lock::{
    hold_descendant_lock, lock_is_held, wait_for_lock_release,
};
// Only the Unix escapee probe builds a bare command, so the helper that does
// so is imported under the same gate; importing it unconditionally makes the
// Windows build fail on an unused import.
#[cfg(unix)]
use super::probe_support::probe_command;
use super::probe_support::{
    RELEASE_FILE_NAME, announce_ready, release_holder, scratch_directory, spawn_owned_probe,
    spawn_probe, wait_for_descendant, wait_for_ready_in, wait_until,
};
use super::supervise;
use anyhow::{Context, Result, ensure};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Deadline given to the probe.
///
/// Long enough that a loaded runner can start the child and let it print,
/// short enough that the regression test does not add a meaningful delay to a
/// suite whose other members take minutes.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// How long the probe sleeps after printing, so it always outlives the deadline.
const PROBE_LIFETIME: Duration = Duration::from_secs(30);

/// How long a surviving descendant waits for the test to release it.
///
/// A descendant that survives the kill must stay alive until the test has
/// asserted that the runner returned while it was still running. Making that
/// causal rather than a race against a sleep is what keeps the regression off
/// the clock: the test always observes a live survivor, then releases it.
const RELEASE_WAIT: Duration = Duration::from_secs(300);

/// How long the runner may take once the scenario is ready.
///
/// The deadline and reader graces account for about fourteen seconds; the rest
/// absorbs a host running a hundred test binaries at once. Waiting for a
/// survivor instead would take at least [`RELEASE_WAIT`], so the two outcomes
/// are separated by more than twenty times this margin.
const COLLECTION_BOUND: Duration = Duration::from_secs(60);

/// Print to both streams, then outlive the deadline under test.
///
/// Ignored so an ordinary run never waits for it; the test below selects it by
/// name when it re-runs this binary as its own child. The writes use `std::io`
/// rather than `println!` because the workspace denies `clippy::print_stdout`
/// and `clippy::print_stderr` in test targets too.
///
/// Readiness is announced only after both streams are flushed, so the test's
/// deadline starts once the output it asserts about is already in the pipes.
/// Otherwise a slow host could consume the whole deadline before the child
/// reached its first line, and the diagnostic assertion would fail for a reason
/// that has nothing to do with the runner.
#[test]
#[ignore = "driven as a child process by deadline_diagnostic_reports_the_child"]
fn deadline_output_probe() -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "probe-stdout-before-deadline")?;
    stdout.flush()?;
    let mut stderr = std::io::stderr().lock();
    writeln!(stderr, "probe-stderr-before-deadline")?;
    stderr.flush()?;
    announce_ready(&scratch_directory()?)?;
    std::thread::sleep(PROBE_LIFETIME);
    Ok(())
}

/// Start a descendant that holds a pipe for the deadline tree probe.
///
/// The descendant inherits this probe's stdout and stderr, which are the
/// runner's pipes, so it models a linker or build script that outlived Cargo.
/// The probe waits for the descendant's lock before outliving its own deadline.
#[test]
#[ignore = "driven as a child process by deadline_kills_the_whole_process_tree"]
fn tree_probe() -> Result<()> {
    let mut descendant = spawn_probe("tree_descendant")?;
    wait_for_descendant()?;
    std::thread::sleep(PROBE_LIFETIME);
    // Only reached if the runner failed to kill the tree.
    let _killed = descendant.kill();
    Ok(())
}

/// Hold a lock and a pipe for longer than the deadline.
#[test]
#[ignore = "driven as a child process by the descending probes"]
fn tree_descendant() -> Result<()> {
    let _lock = hold_descendant_lock()?;
    std::thread::sleep(PROBE_LIFETIME);
    Ok(())
}

/// Exit normally while a descendant still holds this probe's pipes.
///
/// This is the normal-exit half of the pipe defect: the direct child finishes,
/// but its output never reaches end of file because the descendant inherited
/// the pipes. Reader collection must stay bounded in that state.
#[test]
#[ignore = "driven as a child process by a_descendant_holding_the_pipe_cannot_block_collection"]
fn held_pipe_probe() -> Result<()> {
    let _descendant = spawn_probe("pipe_holder")?;
    wait_for_descendant()?;
    Ok(())
}

/// Hold a lock and the inherited pipes until the test releases it.
///
/// Waiting for a release marker rather than sleeping keeps the test off the
/// clock. `hold_descendant_lock` also announces readiness, so the parent's
/// deadline starts only after the descendant is holding the pipe, and the
/// parent can then assert that the survivor was still alive when the runner
/// returned rather than racing a fixed sleep.
#[test]
#[ignore = "driven as a child process by held_pipe_probe"]
fn pipe_holder() -> Result<()> {
    let _lock = hold_descendant_lock()?;
    let scratch = scratch_directory()?;
    wait_until(RELEASE_WAIT, "the pipe holder to be released", || {
        Ok(scratch.exists(RELEASE_FILE_NAME))
    })
}

/// Start a descendant that leaves the process group the runner will kill.
///
/// This models the worst case for tree termination: a process the child started
/// which the group- or Job-Object-directed kill cannot reach. Its pipes stay
/// open after the kill, so only the runner's bounded reader collection can make
/// the deadline reportable.
#[cfg(unix)]
#[test]
#[ignore = "driven as a child process by a_timeout_is_not_blocked_by_a_pipe_holding_escapee"]
fn escapee_probe() -> Result<()> {
    let mut command = probe_command("escapee_holder")?;
    // A new process group takes the holder outside the one the runner kills.
    command.process_group(0);
    let _escapee = command.spawn().context("start the escaped descendant")?;
    wait_for_descendant()?;
    std::thread::sleep(PROBE_LIFETIME);
    Ok(())
}

/// Hold a lock and the inherited pipes until the test releases it.
///
/// The release marker matters: this process survives the runner's kill, so
/// without a way out it would linger past the test binary's own exit.
#[cfg(unix)]
#[test]
#[ignore = "driven as a child process by escapee_probe"]
fn escapee_holder() -> Result<()> {
    let _lock = hold_descendant_lock()?;
    let scratch = scratch_directory()?;
    wait_until(RELEASE_WAIT, "the escapee to be released", || {
        Ok(scratch.exists(RELEASE_FILE_NAME))
    })
}

/// A timed-out child's own output must appear in the error.
///
/// The deadline starts after the probe reports that it has flushed both
/// streams, so the assertion is about retention rather than about the host
/// starting a nested test binary quickly.
#[test]
fn deadline_diagnostic_reports_the_child() -> Result<()> {
    let (_scratch, error) =
        fail_after_readiness("deadline_output_probe", PROBE_DEADLINE, "deadline probe")?;
    let message = format!("{error:#}");
    for expected in [
        "deadline probe exceeded",
        "probe-stdout-before-deadline",
        "probe-stderr-before-deadline",
    ] {
        ensure!(
            message.contains(expected),
            "the deadline error omits {expected:?}:\n{message}",
        );
    }
    Ok(())
}

/// A timeout must kill the descendants the child started, not only the child.
///
/// Readiness is signalled by the descendant once it holds the lock, so the
/// deadline is not charged for the time the host takes to start two nested
/// copies of this binary.
#[test]
fn deadline_kills_the_whole_process_tree() -> Result<()> {
    let (scratch, error) = fail_after_readiness("tree_probe", PROBE_DEADLINE, "tree probe")?;
    ensure!(
        format!("{error:#}").contains("tree probe exceeded"),
        "the error should name the timed-out probe:\n{error:#}",
    );
    // The lock is held until every process in the tree is gone, so acquiring it
    // proves the descendant died with its parent.
    wait_for_lock_release(scratch.path())
}

/// Spawn a probe, wait for its readiness marker, and require supervision to fail.
///
/// The deadline is applied only after the probe says it is ready, so a
/// saturated host's start-up delay is never charged to the runner. The returned
/// directory must stay in scope while the caller still needs the probe's
/// descendants: they use it to take their lock and to learn when to exit.
fn fail_after_readiness(
    filter: &str,
    deadline: Duration,
    operation: &str,
) -> Result<(TempDir, anyhow::Error)> {
    let scratch =
        TempDir::new().with_context(|| format!("create the {filter} scratch directory"))?;
    let child = spawn_owned_probe(filter, scratch.path())?;
    wait_for_ready_in(scratch.path())?;
    match supervise(child, deadline, operation) {
        Err(error) => Ok((scratch, error)),
        Ok(output) => anyhow::bail!(
            "{operation} should have failed; it exited with {:?}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        ),
    }
}

/// A descendant that keeps a pipe open must not block reader collection.
///
/// The direct child exits normally here, so only the bounded reader collection
/// can make the still-open pipe reportable.
#[test]
fn a_descendant_holding_the_pipe_cannot_block_collection() -> Result<()> {
    assert_survivor_is_reported(
        "held_pipe_probe",
        PROBE_DEADLINE * 4,
        "held-pipe probe",
        "pipe remained open",
    )
}

/// A timeout must be reportable even when a descendant escaped the kill.
#[cfg(unix)]
#[test]
fn a_timeout_is_not_blocked_by_a_pipe_holding_escapee() -> Result<()> {
    assert_survivor_is_reported(
        "escapee_probe",
        PROBE_DEADLINE,
        "escapee probe",
        "escapee probe exceeded",
    )
}

/// Run a probe whose descendant survives, and assert it is reported promptly.
///
/// `expected` names the diagnostic that proves the right path was taken: a
/// still-open pipe after a normal exit, or a deadline that expired. The
/// survivor is released only after the assertions, so a runner that waited for
/// it could not return inside [`COLLECTION_BOUND`]; checking `lock_is_held`
/// before releasing makes that causal rather than a race against a clock, and
/// shows the runner gave up while the descendant was genuinely alive.
fn assert_survivor_is_reported(
    filter: &str,
    deadline: Duration,
    operation: &str,
    expected: &str,
) -> Result<()> {
    let scratch =
        TempDir::new().with_context(|| format!("create the {filter} scratch directory"))?;
    let child = spawn_owned_probe(filter, scratch.path())?;
    wait_for_ready_in(scratch.path())?;
    let started = Instant::now();
    let error = match supervise(child, deadline, operation) {
        Err(error) => error,
        Ok(output) => anyhow::bail!(
            "{operation} should be reported while its descendant holds the pipe; \
             it exited with {:?}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        ),
    };
    let elapsed = started.elapsed();
    ensure!(
        elapsed < COLLECTION_BOUND,
        "the runner waited for the descendant instead of bounding its readers: {elapsed:?}",
    );
    ensure!(
        format!("{error:#}").contains(expected),
        "the error should report {expected:?}:\n{error:#}",
    );
    // Non-vacuity: the descendant must have outlived its parent. If the lock
    // were free, the test would prove only that the runner handled a closed
    // pipe or an already-dead process.
    ensure!(
        lock_is_held(scratch.path())?,
        "the descendant should still be alive when the runner returns",
    );
    release_holder(scratch.path())?;
    wait_for_lock_release(scratch.path())
}
