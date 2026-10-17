//! Bounded subprocess execution for executable documentation tests.
//!
//! This module owns time and capture limits only. Callers remain responsible
//! for command construction, environment isolation, and exit-status policy.
//!
//! The timeout is sized from this repository's own runs rather than chosen.
//! The callers that dominate it spawn a child `cargo` in a target directory
//! kept to itself, so they pay a cold dependency build that no in-process
//! test pays, and on a shared build host that build queues behind every
//! other compiler running there. On a `build-test` runner the test took
//! 56.5 s and 120.7 s; on a developer host it took 588.0 s and 593.6 s in
//! two runs, then crossed the old 600 s bound while still building. That
//! is 0.99 of its budget, past the 0.95 that "Test timeouts" in
//! `docs/developers-guide.md` calls already too small rather than merely
//! unlucky. A bound a legitimate build reaches is measuring host load
//! rather than catching a hang, so it is doubled now, to twice the worst
//! run that finished.
//!
//! Under `make test` this is the only timer on the child. The coverage lane
//! runs nextest instead, where the per-test `slow-timeout` bounds the whole
//! test more tightly than this does and its runners are not shared by other
//! builds, which is why the two runner figures above stay under this one.
//!
//! Shared-host compiler admission can exceed even that cold-build allowance.
//! `ORTHO_CONFIG_DOC_TEST_TIMEOUT_SECS` selects a larger, still bounded budget
//! for these subprocesses without changing CI defaults or output capture.

use anyhow::{Context, Result, anyhow, ensure};
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wait_timeout::ChildExt;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1_200);
const DEFAULT_OUTPUT_BYTES: usize = 1024 * 1024;
/// Optional shared-host budget; callers must select it before starting tests.
const TIMEOUT_ENV: &str = "ORTHO_CONFIG_DOC_TEST_TIMEOUT_SECS";
/// Keep overrides finite and safely within platform timer representations.
const MAX_TIMEOUT_SECONDS: u64 = 86_400;

#[derive(Clone, Copy)]
pub(super) struct Operation<'a>(pub(super) &'a str);

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Clone, Copy)]
struct ProcessLimits {
    timeout: Duration,
    output_bytes: usize,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            output_bytes: DEFAULT_OUTPUT_BYTES,
        }
    }
}

impl ProcessLimits {
    /// Read the subprocess budget without mutating the test process environment.
    fn from_environment() -> Result<Self> {
        match std::env::var(TIMEOUT_ENV) {
            Ok(value) => Self::with_timeout_override(Some(&value)),
            Err(std::env::VarError::NotPresent) => Self::with_timeout_override(None),
            Err(error) => Err(error).with_context(|| format!("read {TIMEOUT_ENV}")),
        }
    }

    /// Validate a supplied seconds budget while preserving the capture bound.
    ///
    /// For example, `Some("7200")` permits a two-hour queued build; absence
    /// preserves the twenty-minute default. Invalid input fails before spawn.
    fn with_timeout_override(value: Option<&str>) -> Result<Self> {
        let Some(seconds_text) = value else {
            return Ok(Self::default());
        };
        let seconds = seconds_text
            .parse::<u64>()
            .with_context(|| format!("{TIMEOUT_ENV} must contain whole seconds"))?;
        ensure!(
            (1..=MAX_TIMEOUT_SECONDS).contains(&seconds),
            "{TIMEOUT_ENV} must be between 1 and {MAX_TIMEOUT_SECONDS} seconds"
        );
        Ok(Self {
            timeout: Duration::from_secs(seconds),
            ..Self::default()
        })
    }
}

/// Run a command with the documentation-test timeout and output limits.
pub(super) fn run_command(command: &mut Command, operation: Operation<'_>) -> Result<Output> {
    run_command_with_limits(command, operation, ProcessLimits::from_environment()?)
}

fn run_command_with_limits(
    command: &mut Command,
    operation: Operation<'_>,
    limits: ProcessLimits,
) -> Result<Output> {
    let Operation(operation_name) = operation;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("{operation_name}: start subprocess"))?;
    let stdout_pipe = take_stdout(&mut child, operation)?;
    let stderr_pipe = take_stderr(&mut child, operation)?;
    let stdout_reader = spawn_bounded_reader(stdout_pipe, limits.output_bytes);
    let stderr_reader = spawn_bounded_reader(stderr_pipe, limits.output_bytes);

    let status = wait_for_exit(&mut child, operation, limits.timeout);
    if status.is_err() {
        terminate_child(&mut child, operation)?;
    }
    let captured_stdout = join_reader(stdout_reader, operation, OutputStream::Stdout)?;
    let captured_stderr = join_reader(stderr_reader, operation, OutputStream::Stderr)?;

    Ok(Output {
        status: status?,
        stdout: captured_stdout,
        stderr: captured_stderr,
    })
}

fn take_stdout(
    child: &mut Child,
    Operation(operation): Operation<'_>,
) -> Result<std::process::ChildStdout> {
    child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("{operation}: capture subprocess stdout"))
}

fn take_stderr(
    child: &mut Child,
    Operation(operation): Operation<'_>,
) -> Result<std::process::ChildStderr> {
    child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("{operation}: capture subprocess stderr"))
}

fn spawn_bounded_reader(
    mut pipe: impl Read + Send + 'static,
    output_limit: usize,
) -> JoinHandle<Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut captured = Vec::with_capacity(output_limit.min(8192));
        let mut buffer = [0_u8; 8192];
        loop {
            let bytes_read = pipe.read(&mut buffer).context("read subprocess output")?;
            if bytes_read == 0 {
                break;
            }
            let bytes_to_capture = output_limit.saturating_sub(captured.len()).min(bytes_read);
            captured.extend(buffer.iter().take(bytes_to_capture).copied());
        }
        Ok(captured)
    })
}

fn wait_for_exit(
    child: &mut Child,
    Operation(operation): Operation<'_>,
    timeout: Duration,
) -> Result<ExitStatus> {
    child
        .wait_timeout(timeout)
        .with_context(|| format!("{operation}: wait for subprocess"))?
        .ok_or_else(|| {
            anyhow!(
                "{operation}: subprocess timed out after {}s",
                timeout.as_secs()
            )
        })
}

fn terminate_child(child: &mut Child, Operation(operation): Operation<'_>) -> Result<()> {
    if child
        .try_wait()
        .with_context(|| format!("{operation}: poll timed-out subprocess"))?
        .is_some()
    {
        return Ok(());
    }
    child
        .kill()
        .with_context(|| format!("{operation}: kill timed-out subprocess"))?;
    child
        .wait()
        .with_context(|| format!("{operation}: reap timed-out subprocess"))?;
    Ok(())
}

fn join_reader(
    reader: JoinHandle<Result<Vec<u8>>>,
    Operation(operation): Operation<'_>,
    stream: OutputStream,
) -> Result<Vec<u8>> {
    let stream_name = stream.name();
    reader
        .join()
        .map_err(|_| anyhow!("{operation}: {stream_name} reader thread panicked"))?
}

#[cfg(test)]
mod tests {
    //! Regression coverage for subprocess output and duration bounds.

    use super::{Operation, OutputStream, ProcessLimits, join_reader, run_command_with_limits};
    use anyhow::{Context, Result};
    use proptest::prelude::*;
    use rstest::rstest;
    use std::io::Write;
    use std::process::Command;
    use std::time::Duration;

    /// Budget selection leaves the default and capture limit unchanged.
    #[rstest]
    #[case::default(None, 1_200)]
    #[case::smallest_budget(Some("1"), 1)]
    #[case::shared_host(Some("7200"), 7_200)]
    #[case::largest_budget(Some("86400"), 86_400)]
    fn timeout_override_preserves_output_limit(#[case] value: Option<&str>, #[case] seconds: u64) {
        let limits = ProcessLimits::with_timeout_override(value).expect("valid timeout budget");
        assert_eq!(limits.timeout, Duration::from_secs(seconds));
        assert_eq!(limits.output_bytes, super::DEFAULT_OUTPUT_BYTES);
    }

    /// Invalid budgets fail before a subprocess can be started.
    #[rstest]
    #[case::empty("")]
    #[case::zero("0")]
    #[case::negative("-1")]
    #[case::not_seconds("forever")]
    #[case::over_limit("86401")]
    #[case::overflow("18446744073709551616")]
    fn invalid_timeout_override_is_rejected(#[case] value: &str) {
        assert!(ProcessLimits::with_timeout_override(Some(value)).is_err());
    }

    proptest! {
        /// Accepted budgets retain the requested duration and bounded capture.
        #[test]
        fn timeout_override_preserves_requested_budget(
            seconds in 1..=super::MAX_TIMEOUT_SECONDS,
        ) {
            let seconds_text = seconds.to_string();
            let limits = ProcessLimits::with_timeout_override(Some(&seconds_text))
                .expect("valid timeout budget");
            prop_assert_eq!(limits.timeout.as_secs(), seconds);
            prop_assert_eq!(limits.output_bytes, super::DEFAULT_OUTPUT_BYTES);
        }
    }

    #[test]
    fn output_capture_is_limited_per_stream() {
        let mut command = probe_command("bounded_output_probe")
            .expect("the bounded-output probe command should be available");
        let output = run_command_with_limits(
            &mut command,
            Operation("capture bounded output"),
            ProcessLimits {
                timeout: Duration::from_secs(5),
                output_bytes: 256,
            },
        )
        .expect("bounded-output probe should run");
        assert!(output.status.success(), "probe failed: {output:?}");
        assert_eq!(output.stdout.len(), 256);
        assert_eq!(output.stderr.len(), 256);
    }

    #[test]
    fn stalled_process_is_terminated_at_the_deadline() {
        let mut command = probe_command("bounded_timeout_probe")
            .expect("the bounded-timeout probe command should be available");
        let error = run_command_with_limits(
            &mut command,
            Operation("run timeout probe"),
            ProcessLimits {
                timeout: Duration::from_millis(100),
                output_bytes: 256,
            },
        )
        .expect_err("stalled probe should time out");
        assert!(format!("{error:#}").contains("run timeout probe: subprocess timed out"));
    }

    #[rstest]
    #[case::stdout(OutputStream::Stdout, "read probe: stdout reader thread panicked")]
    #[case::stderr(OutputStream::Stderr, "read probe: stderr reader thread panicked")]
    fn reader_thread_diagnostic_names_the_stream(
        #[case] stream: OutputStream,
        #[case] expected: &str,
    ) {
        let reader = std::thread::spawn(|| -> anyhow::Result<Vec<u8>> {
            panic!("deliberate reader-thread failure")
        });
        let error = join_reader(reader, Operation("read probe"), stream)
            .expect_err("panicked reader thread should return an error");
        assert_eq!(format!("{error:#}"), expected);
    }

    fn probe_command(test_name: &str) -> Result<Command> {
        let executable = std::env::current_exe()
            .context("the integration-test executable should have a path")?;
        let mut command = Command::new(executable);
        command.args(["--ignored", "--nocapture", test_name]);
        Ok(command)
    }

    #[test]
    #[ignore = "executed as a high-output subprocess"]
    fn bounded_output_probe() {
        let output = [b'x'; 4096];
        std::io::stdout()
            .lock()
            .write_all(&output)
            .expect("write stdout probe data");
        std::io::stderr()
            .lock()
            .write_all(&output)
            .expect("write stderr probe data");
    }

    #[test]
    #[ignore = "executed as a stalled subprocess"]
    fn bounded_timeout_probe() {
        std::thread::sleep(Duration::from_secs(30));
    }
}
