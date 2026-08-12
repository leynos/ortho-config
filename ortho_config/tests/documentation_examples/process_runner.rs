//! Bounded subprocess execution for executable documentation tests.
//!
//! This module owns time and capture limits only. Callers remain responsible
//! for command construction, environment isolation, and exit-status policy.

use anyhow::{Context, Result, anyhow};
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wait_timeout::ChildExt;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);
const DEFAULT_OUTPUT_BYTES: usize = 1024 * 1024;

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

/// Run a command with the documentation-test timeout and output limits.
pub(super) fn run_command(command: &mut Command, operation: &str) -> Result<Output> {
    run_command_with_limits(command, operation, ProcessLimits::default())
}

fn run_command_with_limits(
    command: &mut Command,
    operation: &str,
    limits: ProcessLimits,
) -> Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("{operation}: start subprocess"))?;
    let stdout_pipe = take_stdout(&mut child, operation)?;
    let stderr_pipe = take_stderr(&mut child, operation)?;
    let stdout_reader = spawn_bounded_reader(stdout_pipe, limits.output_bytes);
    let stderr_reader = spawn_bounded_reader(stderr_pipe, limits.output_bytes);

    let status = wait_for_exit(&mut child, operation, limits.timeout);
    if status.is_err() {
        terminate_child(&mut child, operation)?;
    }
    let captured_stdout = join_reader(stdout_reader, operation, "stdout")?;
    let captured_stderr = join_reader(stderr_reader, operation, "stderr")?;

    Ok(Output {
        status: status?,
        stdout: captured_stdout,
        stderr: captured_stderr,
    })
}

fn take_stdout(child: &mut Child, operation: &str) -> Result<std::process::ChildStdout> {
    child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("{operation}: capture subprocess stdout"))
}

fn take_stderr(child: &mut Child, operation: &str) -> Result<std::process::ChildStderr> {
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

fn wait_for_exit(child: &mut Child, operation: &str, timeout: Duration) -> Result<ExitStatus> {
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

fn terminate_child(child: &mut Child, operation: &str) -> Result<()> {
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
    operation: &str,
    stream: &str,
) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("{operation}: {stream} reader thread panicked"))?
}

#[cfg(test)]
mod tests {
    //! Regression coverage for subprocess output and duration bounds.

    use super::{ProcessLimits, run_command_with_limits};
    use std::io::Write;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn output_capture_is_limited_per_stream() {
        let mut command = probe_command("bounded_output_probe");
        let output = run_command_with_limits(
            &mut command,
            "capture bounded output",
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
        let mut command = probe_command("bounded_timeout_probe");
        let error = run_command_with_limits(
            &mut command,
            "run timeout probe",
            ProcessLimits {
                timeout: Duration::from_millis(100),
                output_bytes: 256,
            },
        )
        .expect_err("stalled probe should time out");
        assert!(format!("{error:#}").contains("run timeout probe: subprocess timed out"));
    }

    fn probe_command(test_name: &str) -> Command {
        let mut command = Command::new(
            std::env::current_exe().expect("the integration-test executable should have a path"),
        );
        command.args(["--ignored", "--nocapture", test_name]);
        command
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
