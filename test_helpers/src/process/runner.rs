//! Deadline-bounded subprocess execution shared by workspace tests.

use super::{capture, spawn_guard, tree};

use anyhow::{Context, Result, anyhow, bail, ensure};
use process_wrap::std::{ChildWrapper, CommandWrap};
use std::io;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Interval between nonblocking child and reader polls.
const POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Bounded time reserved to terminate the process tree and drain readers.
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
/// Preserve all command output for callers that report Cargo diagnostics.
const OUTPUT_LIMIT: usize = usize::MAX;

/// The operation deadline and labels used in timeout diagnostics.
#[derive(Clone, Copy)]
struct CommandDeadline<'a> {
    /// Monotonic instant by which the command must exit.
    at: Instant,
    /// Configured operation budget, for the diagnostic message.
    duration: Duration,
    /// Whether the budget came from the command or its enclosing sequence.
    scope: &'a str,
    /// Human-readable command description for error messages.
    description: &'a str,
}

/// Run a subprocess under its own deadline.
///
/// Output is drained concurrently, the root command owns its spawned process
/// tree, and cleanup gets a bounded two-second allowance after timeout or
/// completion. The returned [`Output`] retains both streams.
///
/// # Errors
///
/// Returns an error if the command cannot be spawned, a reader fails, the
/// deadline expires, or process-tree and reader cleanup cannot be confirmed.
pub fn run_with_deadline(
    command: Command,
    command_deadline: Duration,
    description: &str,
) -> Result<Output> {
    run_with_optional_sequence_deadline(command, command_deadline, None, description)
}

/// Run a subprocess under its command deadline and an enclosing sequence
/// deadline.
///
/// The earlier remaining deadline applies. Time spent before this function is
/// called, including waiting for a caller-owned lock, is already charged to
/// `sequence_deadline`.
///
/// # Errors
///
/// Returns an error if the command cannot start before its enclosing deadline,
/// cannot be spawned, a reader fails, or process-tree and reader cleanup cannot
/// be confirmed within the reserved cleanup allowance.
pub fn run_with_deadline_until(
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

/// Select the effective command budget and run one serialized child process.
fn run_with_optional_sequence_deadline(
    command: Command,
    command_deadline: Duration,
    sequence_deadline: Option<Instant>,
    description: &str,
) -> Result<Output> {
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

    run_command(command, deadline, timeout_scope, description)
}

/// Spawn one owned process tree, capture its streams, and enforce its deadline.
fn run_command(
    mut command: Command,
    deadline: Duration,
    timeout_scope: &str,
    description: &str,
) -> Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_owned(command).with_context(|| format!("spawn {description}"))?;
    let mut readers = Vec::with_capacity(2);
    if let Err(error) = start_readers(child.as_mut(), &mut readers, description) {
        let primary = error.context(format!("prepare output readers for {description}"));
        return Err(finish_failure(primary, child.as_mut(), &mut readers));
    }

    let Some(operation_deadline) = Instant::now().checked_add(deadline) else {
        return Err(finish_failure(
            anyhow!("{description} deadline is out of range"),
            child.as_mut(),
            &mut readers,
        ));
    };
    let status = match wait_for_process(
        child.as_mut(),
        &mut readers,
        CommandDeadline {
            at: operation_deadline,
            duration: deadline,
            scope: timeout_scope,
            description,
        },
    ) {
        Ok(status) => status,
        Err(error) => return Err(finish_failure(error, child.as_mut(), &mut readers)),
    };

    let reader_deadline = match cleanup_deadline() {
        Ok(cleanup_at) => cleanup_at,
        Err(error) => return Err(finish_failure(error, child.as_mut(), &mut readers)),
    };
    wait_for_readers(&mut readers, reader_deadline);
    if let Some(reader) = readers.iter().find(|reader| reader.was_cancelled()) {
        return Err(anyhow!(
            "{description}: {} output reader was cancelled before EOF",
            reader.stream()
        ));
    }
    if let Some(error) = first_reader_error(&readers) {
        return Err(anyhow!("{description}: {error}"));
    }
    if readers.iter().any(|reader| !reader.is_finished()) {
        request_readers_to_stop(&readers);
        join_readers_until(&mut readers, reader_deadline);
        return Err(reader_cleanup_error(description, &mut readers));
    }

    Ok(Output {
        status,
        stdout: reader_bytes(&readers, "stdout"),
        stderr: reader_bytes(&readers, "stderr"),
    })
}

/// Install shared tree ownership while guarding failures during wrapper setup.
fn spawn_owned(command: Command) -> Result<Box<dyn ChildWrapper>> {
    let mut wrapped_command = CommandWrap::from(command);
    let guard = spawn_guard::SpawnGuard::new();
    guard.install(&mut wrapped_command);
    tree::configure(&mut wrapped_command);
    match wrapped_command.spawn() {
        Ok(child) => {
            guard.disarm();
            Ok(child)
        }
        Err(spawn_error) if guard.root_child_seen() => {
            let evidence = guard.kill_error().map_or_else(
                || "root child termination was requested; reaping is unconfirmed".to_owned(),
                |kill_error| {
                    format!("root child termination failed ({kill_error}); reaping is unconfirmed")
                },
            );
            Err(anyhow!(spawn_error)
                .context("process-tree wrapper setup failed")
                .context(evidence))
        }
        Err(spawn_error) => Err(spawn_error.into()),
    }
}

/// Start one cancellable reader for each captured process stream.
fn start_readers(
    child: &mut dyn ChildWrapper,
    readers: &mut Vec<capture::Reader>,
    description: &str,
) -> Result<()> {
    let stdout = child
        .stdout()
        .take()
        .with_context(|| format!("capture stdout of {description}"))?;
    let stderr = child
        .stderr()
        .take()
        .with_context(|| format!("capture stderr of {description}"))?;
    readers.push(capture::Reader::spawn(
        "stdout",
        stdout,
        OUTPUT_LIMIT,
        |_| {},
    )?);
    readers.push(capture::Reader::spawn(
        "stderr",
        stderr,
        OUTPUT_LIMIT,
        |_| {},
    )?);
    Ok(())
}

/// Join reader threads that have already exited.
fn refresh_readers(readers: &mut [capture::Reader]) {
    for reader in readers {
        reader.refresh();
    }
}

/// Return the first reader error, if either stream failed.
fn first_reader_error(readers: &[capture::Reader]) -> Option<&str> {
    readers.iter().find_map(capture::Reader::error)
}

/// Wait for both readers to observe EOF until the cleanup deadline.
fn wait_for_readers(readers: &mut [capture::Reader], deadline: Instant) {
    while readers.iter().any(|reader| !reader.is_finished()) && Instant::now() < deadline {
        refresh_readers(readers);
        if readers.iter().any(|reader| !reader.is_finished()) {
            sleep_until(deadline);
        }
    }
    refresh_readers(readers);
}

/// Terminate the owned tree and stop readers after an operation failure.
fn finish_failure(
    primary: anyhow::Error,
    child: &mut dyn ChildWrapper,
    readers: &mut [capture::Reader],
) -> anyhow::Error {
    let deadline = cleanup_deadline().unwrap_or_else(|_| Instant::now());
    let mut cleanup_errors = Vec::new();
    request_tree_termination(child, &mut cleanup_errors);
    let tree_completed = wait_for_tree_and_readers(child, readers, deadline, &mut cleanup_errors);
    if !tree_completed {
        cleanup_errors.push("owned process tree did not finish before the cleanup deadline".into());
    }
    finish_reader_cleanup(readers, deadline, &mut cleanup_errors);
    if cleanup_errors.is_empty() {
        primary
    } else {
        primary.context(format!("cleanup incomplete: {}", cleanup_errors.join("; ")))
    }
}

/// Request termination without masking the operation's original error.
fn request_tree_termination(child: &mut dyn ChildWrapper, errors: &mut Vec<String>) {
    if let Err(error) = child.start_kill()
        && error.kind() != io::ErrorKind::NotFound
    {
        errors.push(format!("process tree termination failed: {error}"));
    }
}

/// Poll tree and reader completion until both finish or the cleanup budget ends.
fn wait_for_tree_and_readers(
    child: &mut dyn ChildWrapper,
    readers: &mut [capture::Reader],
    deadline: Instant,
    errors: &mut Vec<String>,
) -> bool {
    let mut tree_completed = false;
    let mut wait_error_seen = false;
    while Instant::now() < deadline {
        refresh_readers(readers);
        match child.try_wait() {
            Ok(Some(_)) => tree_completed = true,
            Err(error) if !wait_error_seen => {
                wait_error_seen = true;
                errors.push(format!("process tree wait failed: {error}"));
            }
            Ok(None) | Err(_) => {}
        }
        if tree_completed && readers.iter().all(capture::Reader::is_finished) {
            return true;
        }
        sleep_until(deadline);
    }
    tree_completed && readers.iter().all(capture::Reader::is_finished)
}

/// Cancel remaining readers, join within budget, and record unfinished work.
fn finish_reader_cleanup(
    readers: &mut [capture::Reader],
    deadline: Instant,
    errors: &mut Vec<String>,
) {
    request_readers_to_stop(readers);
    join_readers_until(readers, deadline);
    for reader in readers.iter_mut().filter(|reader| !reader.is_finished()) {
        if reader.detach_if_running() {
            errors.push(format!(
                "{} reader did not finish before the cleanup deadline",
                reader.stream()
            ));
        }
    }
    if let Some(error) = first_reader_error(readers) {
        errors.push(error.to_owned());
    }
}

/// Poll a process and its readers until the child exits or its deadline expires.
fn wait_for_process(
    child: &mut dyn ChildWrapper,
    readers: &mut [capture::Reader],
    command_deadline: CommandDeadline<'_>,
) -> Result<std::process::ExitStatus> {
    loop {
        refresh_readers(readers);
        if let Some(error) = first_reader_error(readers) {
            return Err(anyhow!("{}: {error}", command_deadline.description));
        }
        if Instant::now() >= command_deadline.at {
            bail!(
                "{} exceeded the {} second {} deadline",
                command_deadline.description,
                command_deadline.duration.as_secs(),
                command_deadline.scope,
            );
        }
        match child
            .try_wait()
            .with_context(|| format!("wait for {}", command_deadline.description))?
        {
            Some(status) => return Ok(status),
            None => sleep_until(command_deadline.at),
        }
    }
}

/// Request unfinished output readers to stop at their next nonblocking poll.
fn request_readers_to_stop(readers: &[capture::Reader]) {
    for reader in readers.iter().filter(|reader| !reader.is_finished()) {
        reader.request_stop();
    }
}

/// Join each output reader only until the shared cleanup deadline.
fn join_readers_until(readers: &mut [capture::Reader], deadline: Instant) {
    for reader in readers {
        reader.join_until(deadline);
    }
}

/// Describe output readers that did not finish before cleanup expired.
fn reader_cleanup_error(description: &str, readers: &mut [capture::Reader]) -> anyhow::Error {
    let unfinished = readers
        .iter()
        .filter(|reader| !reader.is_finished())
        .map(capture::Reader::stream)
        .collect::<Vec<_>>();
    for reader in readers.iter_mut().filter(|reader| !reader.is_finished()) {
        reader.detach_if_running();
    }
    anyhow!(
        "{description}: output readers exceeded their cleanup deadline ({})",
        unfinished.join(", ")
    )
}

/// Copy captured bytes for a named stream, returning empty output when absent.
fn reader_bytes(readers: &[capture::Reader], stream: &str) -> Vec<u8> {
    readers
        .iter()
        .find(|reader| reader.stream() == stream)
        .map(|reader| reader.snapshot().bytes)
        .unwrap_or_default()
}

/// Calculate the end of the fixed cleanup allowance.
fn cleanup_deadline() -> Result<Instant> {
    Instant::now()
        .checked_add(CLEANUP_TIMEOUT)
        .ok_or_else(|| anyhow!("subprocess cleanup deadline is out of range"))
}

/// Sleep no longer than one poll interval or the remaining deadline.
fn sleep_until(deadline: Instant) {
    thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
}
