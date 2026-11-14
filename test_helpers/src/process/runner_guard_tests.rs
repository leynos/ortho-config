//! Check the runner's cleanup helpers against children that misbehave.
//!
//! A real command cannot be made to fail its pipe capture, refuse a group
//! kill, or report a poll error on demand, so these cases drive the helpers
//! with a scripted [`ChildWrapper`]. The real-command cases live beside them in
//! `runner_failure_tests.rs`; together they cover the supervisor's contract
//! that a failed operation still reports every cleanup defect it met.

use super::deadline::effective_deadline;
use super::{capture, finish_failure, start_readers, wait_for_process};

use anyhow::anyhow;
use process_wrap::std::ChildWrapper;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ChildStderr, ChildStdout, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Long enough that no helper case expires on the clock by accident.
const PROBE_BUDGET: Duration = Duration::from_secs(30);

/// A child whose poll, kill, and capture surfaces answer as the case needs.
#[derive(Debug)]
struct ScriptedChild {
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    poll_error: Option<io::Error>,
    kill_error: Option<io::Error>,
    exit_status: Option<ExitStatus>,
}

impl ScriptedChild {
    /// A child with no captured streams and no scripted failures.
    const fn new() -> Self {
        Self {
            stdout: None,
            stderr: None,
            poll_error: None,
            kill_error: None,
            exit_status: None,
        }
    }

    /// A child whose next poll fails.
    fn with_poll_error() -> Self {
        Self {
            poll_error: Some(io::Error::other("scripted poll failure")),
            ..Self::new()
        }
    }

    /// A child whose group kill fails and which never reports an exit.
    fn with_kill_error() -> Self {
        Self {
            kill_error: Some(io::Error::other("scripted kill failure")),
            ..Self::new()
        }
    }

    /// A child that reports the given exit on every poll.
    fn exited(status: ExitStatus) -> Self {
        Self {
            exit_status: Some(status),
            ..Self::new()
        }
    }

    /// Attach a real, already-piped stdout taken from a spawned probe.
    fn with_stdout(mut self, stdout: ChildStdout) -> Self {
        self.stdout = Some(stdout);
        self
    }
}

impl ChildWrapper for ScriptedChild {
    fn inner(&self) -> &dyn ChildWrapper {
        self
    }

    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        self
    }

    fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
        self
    }

    fn stdout(&mut self) -> &mut Option<ChildStdout> {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut Option<ChildStderr> {
        &mut self.stderr
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.poll_error.take().map_or(Ok(self.exit_status), Err)
    }

    fn start_kill(&mut self) -> io::Result<()> {
        self.kill_error.take().map_or(Ok(()), Err)
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        Err(io::Error::other("the scripted child has no unbounded wait"))
    }
}

/// Spawn a probe whose stdout is piped, returning the child and that stream.
///
/// The spawn cannot be asserted here: `expect` in a test helper outside a
/// `#[test]` function is denied, so the failure travels back to the case.
fn piped_probe(script: &str) -> io::Result<(std::process::Child, ChildStdout)> {
    let mut child = std::process::Command::new("sh")
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("the probe stdout is not piped"))?;
    Ok((child, stdout))
}

/// Resolve a deadline for the scripted cases, leaving the impossible case to
/// the caller so this helper stays total.
fn probe_deadline() -> Option<super::deadline::CommandDeadline<'static>> {
    effective_deadline(PROBE_BUDGET, None, Instant::now(), "probe")
}

#[test]
fn an_unpiped_stdout_is_reported_before_any_reader_starts() {
    let mut child = ScriptedChild::new();
    let mut readers = Vec::new();

    let error = start_readers(&mut child, &mut readers, "probe")
        .expect_err("the child has no captured stdout");

    let message = format!("{error:#}");
    assert!(
        message.contains("capture stdout of probe"),
        "the failure names the missing stream: {message}"
    );
    assert!(readers.is_empty(), "no reader is started without a stream");
}

#[test]
fn an_unpiped_stderr_is_reported_before_any_reader_starts() {
    let (mut probe, stdout) = piped_probe(":").expect("the probe starts");
    let mut child = ScriptedChild::new().with_stdout(stdout);
    let mut readers = Vec::new();

    let error = start_readers(&mut child, &mut readers, "probe")
        .expect_err("the child has no captured stderr");

    let message = format!("{error:#}");
    assert!(
        message.contains("capture stderr of probe"),
        "the failure names the missing stream: {message}"
    );
    assert!(
        readers.is_empty(),
        "both streams are taken before either reader starts"
    );
    probe.wait().expect("the probe exits");
}

#[test]
fn a_poll_failure_is_reported_with_the_command_description() {
    let mut child = ScriptedChild::with_poll_error();

    let error = wait_for_process(
        &mut child,
        &mut [],
        probe_deadline().expect("a probe budget resolves"),
    )
    .expect_err("the scripted poll fails");

    let message = format!("{error:#}");
    assert!(
        message.contains("wait for probe"),
        "the failure names the wait it could not complete: {message}"
    );
}

#[test]
fn a_completed_cleanup_leaves_the_original_error_alone() {
    let mut child = ScriptedChild::exited(ExitStatus::from_raw(0));

    let error = finish_failure(anyhow!("operation failed"), &mut child, &mut []);

    assert_eq!(error.to_string(), "operation failed");
}

#[test]
fn an_incomplete_cleanup_is_appended_to_the_original_error() {
    let mut child = ScriptedChild::with_kill_error();

    let error = finish_failure(anyhow!("operation failed"), &mut child, &mut []);

    let message = format!("{error:#}");
    assert!(
        message.contains("operation failed"),
        "the original failure survives: {message}"
    );
    assert!(
        message.contains("cleanup incomplete"),
        "the failure reports that cleanup did not finish: {message}"
    );
    assert!(
        message.contains("process tree termination failed"),
        "the reported cleanup failure is the kill refusal: {message}"
    );
    assert!(
        message.contains("did not finish before the cleanup deadline"),
        "the tree that never exited is reported too: {message}"
    );
}

#[test]
fn a_reader_that_outlives_the_cleanup_budget_is_detached_and_reported() {
    let (mut probe, stdout) = piped_probe("sleep 5").expect("the probe starts");
    let reader = capture::Reader::spawn("stdout", stdout, usize::MAX, |_| {})
        .expect("the reader starts over the probe pipe");
    let mut readers = vec![reader];
    let mut child = ScriptedChild::exited(ExitStatus::from_raw(0));

    let error = finish_failure(anyhow!("operation failed"), &mut child, &mut readers);

    let message = format!("{error:#}");
    assert!(
        message.contains("stdout reader did not finish before the cleanup deadline"),
        "the unfinished reader is named: {message}"
    );
    probe.kill().expect("the probe is still running");
    probe.wait().expect("the probe is reaped");
}
