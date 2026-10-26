//! Bounded subprocess execution for the boundary Cargo fixtures.

use anyhow::{Context, Result, bail, ensure};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt as _;

/// Run `command` to completion or its individual deadline, whichever comes first.
///
/// Both streams are drained on their own threads. A nested Cargo build prints
/// far more than a pipe buffer holds, and a runner that waits for the exit
/// status before reading the pipes therefore deadlocks: the child blocks
/// writing, the parent blocks waiting, and the deadline reports a hang that is
/// really the harness's own. Draining concurrently removes that failure mode.
///
/// # Errors
///
/// Returns an error when the child cannot be spawned or collected, or when it
/// is still running at the deadline. A timed-out child is killed before the
/// error is returned, so the test process is not left with an orphan holding
/// the package-cache lock.
pub(crate) fn run_with_deadline(
    command: Command,
    command_deadline: Duration,
    description: &str,
) -> Result<Output> {
    run_with_optional_sequence_deadline(command, command_deadline, None, description)
}

/// Run a child command under both its own and an enclosing sequence deadline.
///
/// The local Cargo lock is acquired before the remaining sequence time is
/// measured, so waiting behind a sibling command consumes the shared budget.
///
/// # Errors
///
/// Returns an error when the child cannot be spawned or collected, or either
/// deadline expires. A timed-out child is killed before returning.
pub(crate) fn run_with_deadline_until(
    command: Command,
    command_deadline: Duration,
    sequence_deadline: Instant,
    description: &str,
) -> Result<Output> {
    run_with_optional_sequence_deadline(
        command,
        command_deadline,
        Some(sequence_deadline),
        description,
    )
}

fn run_with_optional_sequence_deadline(
    mut command: Command,
    command_deadline: Duration,
    sequence_deadline: Option<Instant>,
    description: &str,
) -> Result<Output> {
    // `cargo test` runs this binary's cases concurrently, but their nested
    // commands share one Cargo target and package cache. Serialize the children
    // before starting each command deadline. A sequence deadline, when set,
    // already includes any time spent waiting for this lock.
    static CARGO_COMMAND_LOCK: Mutex<()> = Mutex::new(());
    let _cargo_guard = CARGO_COMMAND_LOCK
        .lock()
        .map_err(|error| anyhow::anyhow!("nested Cargo command lock was poisoned: {error}"))?;

    let (deadline, timeout_scope) =
        sequence_deadline.map_or((command_deadline, "command"), |sequence| {
            let remaining = sequence.saturating_duration_since(Instant::now());
            if remaining < command_deadline {
                (remaining, "shared sequence")
            } else {
                (command_deadline, "command")
            }
        });
    ensure!(
        !deadline.is_zero(),
        "{description} could not start before its enclosing sequence deadline"
    );

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {description}"))?;
    let stdout_reader = drain(
        child.stdout.take(),
        &format!("read stdout of {description}"),
    )?;
    let stderr_reader = drain(
        child.stderr.take(),
        &format!("read stderr of {description}"),
    )?;

    let waited = child
        .wait_timeout(deadline)
        .with_context(|| format!("wait for {description}"))?;
    let Some(status) = waited else {
        // Reap the child: a killed process releases its package-cache lock and
        // its compile-admission slot, which a leaked one would hold.
        let _killed = child.kill();
        let _reaped = child.wait();
        bail!(
            "{description} exceeded the {} second {timeout_scope} deadline",
            deadline.as_secs(),
        );
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("the stdout reader of {description} panicked"))?
        .with_context(|| format!("read stdout of {description}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("the stderr reader of {description} panicked"))?
        .with_context(|| format!("read stderr of {description}"))?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Start a thread that reads `handle` to end of file.
fn drain<R: Read + Send + 'static>(
    handle: Option<R>,
    description: &str,
) -> Result<std::thread::JoinHandle<std::io::Result<Vec<u8>>>> {
    let mut stream = handle.with_context(|| format!("capture {description}"))?;
    std::thread::Builder::new()
        .name(description.to_owned())
        .spawn(move || {
            let mut buffer = Vec::new();
            stream.read_to_end(&mut buffer)?;
            Ok(buffer)
        })
        .with_context(|| format!("start the reader for {description}"))
}
