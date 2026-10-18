//! Bounded subprocess execution for executable documentation tests.
//!
//! This module owns subprocess trees, operation deadlines, and output capture.
//! Callers remain responsible for command construction and environment policy.

#[path = "process_runner_capture.rs"]
mod capture;
#[path = "process_runner_cleanup.rs"]
mod cleanup;
#[path = "process_runner_diagnostics.rs"]
mod diagnostics;
#[cfg(unix)]
#[path = "process_runner_pipe_unix.rs"]
mod pipe;
#[cfg(windows)]
#[path = "process_runner_pipe_windows.rs"]
mod pipe;
#[path = "process_runner_policy.rs"]
mod process_policy;
#[path = "process_runner_spawn_guard.rs"]
mod spawn_guard;
#[cfg(unix)]
#[path = "process_runner_tree_unix.rs"]
mod tree;
#[cfg(windows)]
#[path = "process_runner_tree_windows.rs"]
mod tree;

use diagnostics::{capture_snapshots, environment_evidence, stream_bytes, workspace_identity};
pub(crate) use process_policy::Policy;

use anyhow::{Context, Result, anyhow};
use capture::Reader;
use process_wrap::std::{ChildWrapper, CommandWrap};
use spawn_guard::SpawnGuard;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(10);

struct RunContext {
    operation: String,
    workspace: String,
    environment: String,
    started: Instant,
    policy: Policy,
    process_id: u32,
}

struct RunHooks<Observer, Poll, Kill> {
    observer: Observer,
    poll: Poll,
    kill: Kill,
}

struct WaitProcess<'a> {
    child: &'a mut dyn ChildWrapper,
    readers: &'a mut [Reader],
    operation: &'a str,
    deadline: Instant,
    timeout: Duration,
    status: Option<ExitStatus>,
}

#[derive(Clone, Copy)]
pub(super) struct Operation<'a>(pub(super) &'a str);

#[derive(Debug)]
struct IncompleteCleanup;

#[derive(Debug)]
struct DeadlineExceeded {
    operation: String,
    reason: &'static str,
    timeout: Duration,
}

impl fmt::Display for DeadlineExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} after {}ms",
            self.operation,
            self.reason,
            self.timeout.as_millis()
        )
    }
}

impl StdError for DeadlineExceeded {}

/// Identify deadline exhaustion without parsing a diagnostic string.
pub(crate) fn deadline_exceeded(error: &anyhow::Error) -> bool {
    error.downcast_ref::<DeadlineExceeded>().is_some()
}

impl fmt::Display for IncompleteCleanup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("owned subprocess cleanup incomplete")
    }
}

impl StdError for IncompleteCleanup {}

/// Report whether an error left owned subprocess cleanup unconfirmed.
pub(crate) fn cleanup_incomplete(error: &anyhow::Error) -> bool {
    error.downcast_ref::<IncompleteCleanup>().is_some()
}

/// Run a command while applying an explicit operation and cleanup policy.
pub(crate) fn run_with_policy(
    command: &mut Command,
    operation: Operation<'_>,
    policy: Policy,
) -> Result<Output> {
    run_with_observer(command, operation, policy, |_| {})
}

/// Run a command and observe stdout chunks while they are drained.
///
/// The observer sees all stdout bytes, including bytes beyond the retained
/// capture limit. It must return promptly because it shares the drain thread.
pub(crate) fn run_with_observer<Observer>(
    command: &mut Command,
    operation: Operation<'_>,
    policy: Policy,
    observer: Observer,
) -> Result<Output>
where
    Observer: FnMut(&[u8]) + Send + 'static,
{
    run_with_hooks(
        command,
        operation,
        policy,
        RunHooks {
            observer,
            poll: poll_child,
            kill: kill_child,
        },
    )
}

fn poll_child(child: &mut dyn ChildWrapper) -> io::Result<Option<ExitStatus>> {
    child.try_wait()
}

fn kill_child(child: &mut dyn ChildWrapper) -> io::Result<()> {
    child.start_kill()
}

fn run_with_hooks<Observer, Poll, Kill>(
    command: &mut Command,
    Operation(operation_name): Operation<'_>,
    policy: Policy,
    hooks: RunHooks<Observer, Poll, Kill>,
) -> Result<Output>
where
    Observer: FnMut(&[u8]) + Send + 'static,
    Poll: FnMut(&mut dyn ChildWrapper) -> io::Result<Option<ExitStatus>>,
    Kill: FnMut(&mut dyn ChildWrapper) -> io::Result<()>,
{
    let RunHooks {
        observer,
        poll,
        mut kill,
    } = hooks;
    let started = Instant::now();
    let deadline = started
        .checked_add(policy.timeout())
        .ok_or_else(|| anyhow!("{operation_name}: operation deadline is out of range"))?;
    let workspace = workspace_identity(command);
    let environment = environment_evidence(command);

    let mut child =
        spawn_captured(command).with_context(|| format!("{operation_name}: start subprocess"))?;
    let process_id = child.id();
    let context = RunContext {
        operation: operation_name.to_owned(),
        workspace,
        environment,
        started,
        policy,
        process_id,
    };
    let mut readers = Vec::with_capacity(2);
    if let Err(error) = spawn_readers(
        child.as_mut(),
        &mut readers,
        policy.output_limit(),
        observer,
    ) {
        return Err(cleanup::finish_failure(
            error.context(format!("{operation_name}: prepare output readers")),
            cleanup::CleanupProcess {
                child: &mut child,
                readers: &mut readers,
                kill: &mut kill,
                context: &context,
            },
        ));
    }

    let status = match wait_for_exit(
        WaitProcess {
            child: child.as_mut(),
            readers: &mut readers,
            operation: operation_name,
            deadline,
            timeout: policy.timeout(),
            status: None,
        },
        poll,
    ) {
        Ok(exit_status) => exit_status,
        Err(error) => {
            return Err(cleanup::finish_failure(
                error,
                cleanup::CleanupProcess {
                    child: &mut child,
                    readers: &mut readers,
                    kill: &mut kill,
                    context: &context,
                },
            ));
        }
    };
    let captures = capture_snapshots(&readers);
    Ok(Output {
        status,
        stdout: stream_bytes(&captures, "stdout"),
        stderr: stream_bytes(&captures, "stderr"),
    })
}

fn spawn_captured(command: &mut Command) -> Result<Box<dyn ChildWrapper>> {
    // Transfer the configured command before installing subprocess ownership.
    let mut owned_command = std::mem::replace(command, Command::new(""));
    owned_command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn_owned(owned_command)
}

fn spawn_readers<Observer>(
    child: &mut dyn ChildWrapper,
    readers: &mut Vec<Reader>,
    output_limit: usize,
    observer: Observer,
) -> Result<()>
where
    Observer: FnMut(&[u8]) + Send + 'static,
{
    let stdout = child
        .stdout()
        .take()
        .ok_or_else(|| anyhow!("capture subprocess stdout"))?;
    let stderr = child
        .stderr()
        .take()
        .ok_or_else(|| anyhow!("capture subprocess stderr"))?;
    readers.push(Reader::spawn("stdout", stdout, output_limit, observer)?);
    readers.push(Reader::spawn("stderr", stderr, output_limit, |_| {})?);
    Ok(())
}

fn wait_for_exit<Poll>(mut process: WaitProcess<'_>, mut poll: Poll) -> Result<ExitStatus>
where
    Poll: FnMut(&mut dyn ChildWrapper) -> io::Result<Option<ExitStatus>>,
{
    loop {
        for reader in process.readers.iter_mut() {
            reader.refresh();
        }
        if let Some(reader_error) = process.readers.iter().find_map(|reader| reader.error()) {
            return Err(anyhow!("{}: {reader_error}", process.operation));
        }

        let before_poll = Instant::now();
        if before_poll >= process.deadline {
            return Err(expired_operation_error(&process));
        }

        if process.status.is_none() {
            process.status = poll(process.child)
                .with_context(|| format!("{}: wait for subprocess", process.operation))?;
        }
        let after_poll = Instant::now();
        if after_poll >= process.deadline {
            return Err(expired_operation_error(&process));
        }
        if let Some(exit_status) = process.status
            && process.readers.iter().all(Reader::is_finished)
        {
            return Ok(exit_status);
        }

        thread::sleep(POLL_INTERVAL.min(process.deadline.saturating_duration_since(after_poll)));
    }
}

fn expired_operation_error(process: &WaitProcess<'_>) -> anyhow::Error {
    let reason = match (
        process.status.is_some(),
        process.readers.iter().all(Reader::is_finished),
    ) {
        (true, true) => "subprocess completed after operation deadline",
        (true, false) => "captured output remained open",
        (false, _) => "subprocess timed out",
    };
    DeadlineExceeded {
        operation: process.operation.to_owned(),
        reason,
        timeout: process.timeout,
    }
    .into()
}

fn spawn_owned(command: Command) -> Result<Box<dyn ChildWrapper>> {
    let mut wrapped_command = CommandWrap::from(command);
    let guard = SpawnGuard::new();
    guard.install(&mut wrapped_command);
    #[cfg(any(unix, windows))]
    tree::configure(&mut wrapped_command);
    #[cfg(not(any(unix, windows)))]
    return Err(
        io::Error::other("process-tree supervision is unsupported on this platform").into(),
    );
    #[cfg(any(unix, windows))]
    finish_wrapped_spawn(&mut wrapped_command, &guard)
}

fn finish_wrapped_spawn(
    command: &mut CommandWrap,
    guard: &SpawnGuard,
) -> Result<Box<dyn ChildWrapper>> {
    match command.spawn() {
        Ok(child) => {
            guard.disarm();
            Ok(child)
        }
        Err(spawn_error) if guard.root_child_seen() => {
            let contextual_spawn_error =
                anyhow::Error::new(spawn_error).context("process-tree wrapper setup failed");
            let evidence = guard.kill_error().map_or_else(
                || "root child termination was requested; reaping is unconfirmed".to_owned(),
                |kill_error| {
                    format!("root child termination failed ({kill_error}); reaping is unconfirmed")
                },
            );
            Err(contextual_spawn_error
                .context(evidence)
                .context(IncompleteCleanup))
        }
        Err(unwrapped_spawn_error) => Err(unwrapped_spawn_error.into()),
    }
}

#[cfg(test)]
#[path = "process_runner_tests.rs"]
mod tests;
