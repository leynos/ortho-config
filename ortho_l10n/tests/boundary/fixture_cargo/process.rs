//! Running a nested Cargo child under a deadline, with its output retained.
//!
//! A boundary fixture drives Cargo, and Cargo can spend its whole allowance
//! waiting for a compile-admission slot or a package-cache lock without ever
//! reaching a compiler. This module makes that wait attributable: the child's
//! streams are drained on their own threads, a bounded tail of each is
//! published while it runs, and a deadline error carries that tail. The
//! draining and retention live in [`readers`]; this module owns the child's
//! lifetime.
//!
//! It also owns the child's whole process tree. A nested Cargo build is a tree:
//! `cargo` starts `rustc`, build scripts, and linkers beneath itself. Killing
//! only the direct child leaves those descendants running, still compiling,
//! still holding the output pipes, the package-cache lock, and their
//! compile-admission slots. The runner therefore spawns the child as the leader
//! of its own process group (Unix) or inside its own Job Object (Windows), and
//! terminates that whole tree on the deadline. Reader collection is bounded in
//! both outcomes: after a normal exit a descendant may hold a pipe open, and
//! waiting for that descendant is the same hang by another route.
//!
//! Finally, it serializes nested invocations within the test process. `cargo
//! test` runs a binary's tests as threads, which is the one runner where
//! nextest's `boundary_packaging` group cannot help.

#[cfg(test)]
#[path = "deadline_probe_tests.rs"]
mod deadline_probe_tests;
#[cfg(test)]
#[path = "probe_support.rs"]
mod probe_support;
#[path = "readers.rs"]
mod readers;
#[cfg(test)]
#[path = "serialization_probe_tests.rs"]
mod serialization_probe_tests;
#[cfg(test)]
#[path = "tail_probe_tests.rs"]
mod tail_probe_tests;

use self::readers::{READER_GRACE, StreamCapture, StreamEvidence, drain};
use super::budget::Bound;
use anyhow::{Context, Result};
#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long the runner waits for a killed tree to be reaped.
///
/// `SIGKILL` and `TerminateJobObject` are not blocked by ordinary process
/// cleanup, so this normally completes at once. The bound exists so a process
/// wedged in an uninterruptible kernel wait cannot stall the runner after it
/// has already decided the child timed out.
const REAP_GRACE: Duration = Duration::from_secs(5);

/// How often a running child and its stream readers are polled.
///
/// Ten milliseconds keeps the deadline close to the requested duration without
/// a busy loop; the child and the reader threads are polled at the same
/// cadence, so a bounded wait costs at most one interval beyond its grace.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Serializes nested Cargo invocations within one test process.
///
/// `cargo test` runs a binary's tests as threads in a single process, so the
/// boundary contracts' nested Cargo calls queue on the build-directory lock of
/// the target directory they share. That wait is charged to whichever deadline
/// is counting, which turns a healthy child into a reported hang. nextest
/// already serializes this binary through its `boundary_packaging` group; this
/// gives `cargo test` and `make boundary` the same guarantee. It is acquired
/// before the child is spawned, so time spent waiting for a sibling is never
/// counted against the child's own deadline.
static NESTED_CARGO: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

/// Run `command` to completion or the deadline, whichever comes first.
///
/// Both streams are drained on their own threads. A nested Cargo build prints
/// far more than a pipe buffer holds, and a runner that waits for the exit
/// status before reading the pipes therefore deadlocks: the child blocks
/// writing, the parent blocks waiting, and the deadline reports a hang that is
/// really the harness's own. Draining concurrently removes that failure mode,
/// so a reported deadline means the child itself made no progress.
///
/// Each reader also publishes a bounded tail while it drains, and that tail is
/// part of the deadline error. A child waiting for a compile-admission slot or
/// a package-cache lock prints no compiler diagnostic of its own; the queue and
/// heartbeat lines it does print are the only evidence that distinguishes the
/// wait from a hang. Discarding them made earlier failures unattributable.
///
/// The child is made the root of its own process tree, so a deadline kill ends
/// the compilers it started rather than only the Cargo process that fronted
/// them. Nested invocations are additionally serialized within the process;
/// see [`NESTED_CARGO`].
///
/// The bound comes from [`super::budget`], not from a bare cap, so a contract
/// that makes several calls cannot spend the test's whole allowance on the
/// earlier ones and leave a later invocation to be stopped by nextest. When
/// the shared allowance set the bound, the deadline error says so.
///
/// # Errors
///
/// Returns an error when the child cannot be spawned or collected, when it is
/// still running at the deadline, or when its output remains open after it has
/// exited. A timed-out child's whole tree is killed before the error is
/// returned, so the test process is not left with an orphan holding the
/// package-cache lock. The error carries the bounded tail of both streams.
pub(crate) fn run_with_deadline(
    command: Command,
    bound: Bound,
    description: &str,
) -> Result<Output> {
    let _serialized = NESTED_CARGO.lock().unwrap_or_else(PoisonError::into_inner);
    // Only the deadline error carries the note: a spawn failure should name the
    // command, not repeat why its deadline was shortened.
    let deadline_description = if bound.limited_by_allowance {
        format!("{description} (bounded by the boundary contract's shared allowance)")
    } else {
        description.to_owned()
    };
    let spawned = spawn_owned(command).with_context(|| format!("spawn {description}"))?;
    supervise(spawned, bound.deadline, &deadline_description)
}

/// Supervise a process tree that is already running.
///
/// The deadline is measured from this call, not from the spawn. That matters
/// for the probe regressions: process startup on a saturated host can take tens
/// of seconds, and a deadline that started at spawn would report the host's
/// scheduling delay as the runner failing to bound the child. A caller that
/// needs setup to complete first spawns through [`spawn_owned`], waits for the
/// child's own readiness signal, and only then supervises it.
///
/// Every early return is covered: the guard kills the tree when reader setup,
/// polling, or a panic unwinds before the child was reaped. Without it those
/// paths drop the handle and leave the tree running, which is the orphan
/// condition this module exists to prevent.
///
/// This does not take the [`NESTED_CARGO`] serialization, so a caller that
/// bypasses [`run_with_deadline`] must not supervise a Cargo child.
pub(super) fn supervise(
    child: Box<dyn ChildWrapper>,
    deadline: Duration,
    description: &str,
) -> Result<Output> {
    let mut tree = TreeGuard::new(child);
    let mut stdout_reader = drain(tree.child().stdout().take(), "stdout", description)?;
    let mut stderr_reader = drain(tree.child().stderr().take(), "stderr", description)?;

    let outcome =
        wait_until(tree.child(), deadline).with_context(|| format!("wait for {description}"))?;
    let Some(status) = outcome else {
        // `kill_after_deadline` owns the kill and the reap on this path.
        tree.disarm();
        return Err(kill_after_deadline(
            tree.child(),
            description,
            deadline,
            &mut Streams {
                stdout: &mut stdout_reader,
                stderr: &mut stderr_reader,
            },
        ));
    };
    // Signalling after a reaped leader risks targeting a reused group or Job
    // Object identifier, so the guard lets go as soon as the exit is observed.
    tree.disarm();
    let stdout = stdout_reader
        .finish_after_exit(READER_GRACE)
        .map_err(|error| error.context(format!("collect stdout of {description}")))?;
    let stderr = stderr_reader
        .finish_after_exit(READER_GRACE)
        .map_err(|error| error.context(format!("collect stderr of {description}")))?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Kills the supervised tree if supervision leaves before reaping it.
///
/// The deadline path kills explicitly; the guard covers the paths that return
/// early for another reason, and panics. It is disarmed the moment the child's
/// exit is observed, because the group or Job Object identifier can be reused
/// once the leader has been reaped.
struct TreeGuard {
    /// The tree this guard is responsible for until disarmed.
    child: Box<dyn ChildWrapper>,
    /// Whether dropping the guard should terminate the tree.
    armed: bool,
}

impl TreeGuard {
    /// Take responsibility for `child`, armed.
    fn new(child: Box<dyn ChildWrapper>) -> Self {
        Self { child, armed: true }
    }

    /// The supervised tree.
    fn child(&mut self) -> &mut dyn ChildWrapper {
        self.child.as_mut()
    }

    /// Stop the guard from signalling a child that has already exited.
    const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TreeGuard {
    fn drop(&mut self) {
        if self.armed {
            // The child is still running, so there is nothing to report: the
            // caller is already returning an error or unwinding.
            let _kill_requested = self.child.start_kill();
        }
    }
}

/// Spawn `command` as the root of a process tree this runner owns.
///
/// Unix gets a process group led by the child; Windows gets a Job Object whose
/// close terminates every process assigned to it. Both wrappers make one
/// termination request cover every descendant, including build scripts and
/// linkers that outlive the Cargo process that started them.
///
/// The returned tree must be handed to [`supervise`]; spawning without
/// supervising leaves the child's pipes undrained and unread.
pub(super) fn spawn_owned(mut command: Command) -> Result<Box<dyn ChildWrapper>> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut wrapped = CommandWrap::from(command);
    #[cfg(unix)]
    wrapped.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    wrapped.wrap(JobObject);
    // There is no portable tree to own on any other platform, and a runtime
    // refusal would only defer the same fact to a test failure. Rejecting it at
    // compile time keeps the supported paths free of lint-appeasement reads.
    #[cfg(not(any(unix, windows)))]
    compile_error!("process-tree supervision needs a Unix process group or a Windows Job Object");
    wrapped
        .spawn()
        .map_err(|error| anyhow::Error::new(error).context("start the process tree"))
}

/// Poll `child` until it exits or the deadline passes.
fn wait_until(child: &mut dyn ChildWrapper, deadline: Duration) -> Result<Option<ExitStatus>> {
    let limit = Instant::now() + deadline;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let now = Instant::now();
        if now >= limit {
            return Ok(None);
        }
        std::thread::sleep(POLL_INTERVAL.min(limit.saturating_duration_since(now)));
    }
}

/// A child's two drained streams, as the deadline path needs them.
struct Streams<'a> {
    /// The child's standard output.
    stdout: &'a mut StreamCapture,
    /// The child's standard error.
    stderr: &'a mut StreamCapture,
}

/// Terminate the timed-out tree, retain its output, and build the error.
///
/// The termination request covers the whole group or Job Object, so a compiler
/// started by the child does not keep running after the deadline. Reaping and
/// reader collection are bounded: the diagnostic must be reportable even when
/// a descendant refuses to die.
fn kill_after_deadline(
    child: &mut dyn ChildWrapper,
    description: &str,
    deadline: Duration,
    streams: &mut Streams<'_>,
) -> anyhow::Error {
    let _kill_requested = child.start_kill();
    let _reaped = reap_within(child, REAP_GRACE);
    // Give the readers a bounded chance to finish, so the diagnostic includes
    // everything already written to the pipes.
    streams.stdout.detach_after_kill(READER_GRACE);
    streams.stderr.detach_after_kill(READER_GRACE);
    deadline_failure(
        description,
        deadline,
        &streams.stdout.evidence(),
        &streams.stderr.evidence(),
    )
}

/// Reap a killed child, giving up after `grace` instead of blocking forever.
///
/// A failed wait here is not worth reporting: the caller is already building a
/// deadline failure, and the child was just sent a termination request. The
/// bound exists so a process wedged in an uninterruptible wait cannot stall the
/// diagnostic.
fn reap_within(child: &mut dyn ChildWrapper, grace: Duration) -> Option<ExitStatus> {
    wait_until(child, grace).ok().flatten()
}

/// Build the error for a child that outlived its deadline.
///
/// The retained tails are the substance of the message: without them a
/// deadline failure says only that something took too long, which cannot be
/// told apart from a compile-admission wait, a package-cache wait, or a hang.
fn deadline_failure(
    description: &str,
    deadline: Duration,
    stdout: &StreamEvidence,
    stderr: &StreamEvidence,
) -> anyhow::Error {
    anyhow::anyhow!(
        "{description} exceeded the {} second deadline; its process tree was killed.\n{}\n{}",
        deadline.as_secs(),
        stdout.describe("stdout"),
        stderr.describe("stderr"),
    )
}
