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

#[cfg(test)]
mod tests {
    //! Behavioural tests for the boundary command runner.

    use super::{run_with_deadline, run_with_deadline_until};
    use anyhow::{Context, Result, ensure};
    use cap_std::{ambient_authority, fs::Dir};
    use rstest::rstest;
    use std::io::Write;
    use std::path::Path;
    use std::process::Command;
    use std::thread;
    use std::time::{Duration, Instant};

    const CHILD_MODE_ENV: &str = "ORTHO_L10N_BOUNDARY_COMMAND_TEST_MODE";
    const CHILD_MARKER_ROOT_ENV: &str = "ORTHO_L10N_BOUNDARY_COMMAND_TEST_MARKER_ROOT";
    const STREAM_PAYLOAD_BYTES: usize = 256 * 1024;
    const CHILD_COMPLETION_DELAY: Duration = Duration::from_millis(1_500);

    #[rstest]
    fn boundary_command_runner_child_process_behavior() -> Result<()> {
        let Some(mode) = std::env::var_os(CHILD_MODE_ENV) else {
            return Ok(());
        };

        match mode.to_string_lossy().as_ref() {
            "large-output" => write_large_streams(),
            "delayed-markers" => write_delayed_markers(),
            other => anyhow::bail!("unknown child process test mode: {other}"),
        }
    }

    #[rstest]
    fn runner_drains_large_stdout_and_stderr() -> Result<()> {
        let output = run_with_deadline(
            child_command("large-output", None)?,
            Duration::from_secs(10),
            "large-output child",
        )?;

        ensure!(
            output.status.success(),
            "large-output child failed: {output:?}"
        );
        ensure!(
            output
                .stdout
                .windows(b"BOUNDARY_STDOUT_START".len())
                .any(|window| window == b"BOUNDARY_STDOUT_START"),
            "child stdout marker was not captured"
        );
        ensure!(
            output
                .stderr
                .windows(b"BOUNDARY_STDERR_START".len())
                .any(|window| window == b"BOUNDARY_STDERR_START"),
            "child stderr marker was not captured"
        );
        ensure!(
            output.stdout.len() >= STREAM_PAYLOAD_BYTES,
            "child stdout did not exceed a pipe buffer"
        );
        ensure!(
            output.stderr.len() >= STREAM_PAYLOAD_BYTES,
            "child stderr did not exceed a pipe buffer"
        );
        Ok(())
    }

    #[rstest]
    #[case::shared_sequence(Duration::from_secs(2), Duration::from_millis(750), "shared sequence")]
    #[case::command(Duration::from_millis(750), Duration::from_secs(2), "command")]
    fn runner_kills_timed_out_child(
        #[case] command_deadline: Duration,
        #[case] sequence_budget: Duration,
        #[case] timeout_scope: &str,
    ) -> Result<()> {
        let marker_root = tempfile::tempdir()?;
        let marker_dir = Dir::open_ambient_dir(marker_root.path(), ambient_authority())?;
        let error = run_with_deadline_until(
            child_command("delayed-markers", Some(marker_root.path()))?,
            command_deadline,
            Instant::now() + sequence_budget,
            "delayed-marker child",
        )
        .expect_err("the sleeping child must exceed its selected deadline");

        ensure!(
            format!("{error:#}").contains(timeout_scope),
            "timeout reported the wrong scope: {error:#}"
        );
        ensure!(
            marker_dir.exists("started"),
            "the timed-out child did not start"
        );
        thread::sleep(CHILD_COMPLETION_DELAY + Duration::from_millis(250));
        ensure!(
            !marker_dir.exists("completed"),
            "the timed-out child must not continue running after the runner returns"
        );
        Ok(())
    }

    #[rstest]
    fn expired_sequence_deadline_does_not_spawn_child() -> Result<()> {
        let marker_root = tempfile::tempdir()?;
        let marker_dir = Dir::open_ambient_dir(marker_root.path(), ambient_authority())?;
        let expired_deadline = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .context("calculate an expired sequence deadline")?;

        let error = run_with_deadline_until(
            child_command("delayed-markers", Some(marker_root.path()))?,
            Duration::from_secs(2),
            expired_deadline,
            "expired-deadline child",
        )
        .expect_err("an expired sequence deadline must reject the child");

        ensure!(
            format!("{error:#}").contains("could not start before"),
            "expired sequence deadline reported the wrong error: {error:#}"
        );
        ensure!(
            !marker_dir.exists("started"),
            "expired sequence deadline must not spawn the child"
        );
        Ok(())
    }

    #[rstest]
    fn runner_reports_spawn_errors_with_command_context() -> Result<()> {
        let executable_root = tempfile::tempdir()?;
        let missing_executable = executable_root.path().join("missing-command");
        let error = run_with_deadline_until(
            Command::new(missing_executable),
            Duration::from_secs(2),
            Instant::now() + Duration::from_secs(2),
            "invalid executable",
        )
        .expect_err("a missing executable must be reported as a spawn failure");

        ensure!(
            format!("{error:#}").contains("spawn invalid executable"),
            "spawn failure omitted command context: {error:#}"
        );
        Ok(())
    }

    fn child_command(mode: &str, marker_root: Option<&Path>) -> Result<Command> {
        let executable = std::env::current_exe().context("locate the boundary test binary")?;
        let mut command = Command::new(executable);
        command
            .args([
                "--nocapture",
                "boundary_command_runner_child_process_behavior",
            ])
            .env(CHILD_MODE_ENV, mode);
        if let Some(root) = marker_root {
            command.env(CHILD_MARKER_ROOT_ENV, root);
        }
        Ok(command)
    }

    fn write_large_streams() -> Result<()> {
        let output = vec![b'o'; STREAM_PAYLOAD_BYTES];
        let error = vec![b'e'; STREAM_PAYLOAD_BYTES];

        let stdout_handle = std::io::stdout();
        let mut stdout_writer = stdout_handle.lock();
        stdout_writer.write_all(b"BOUNDARY_STDOUT_START")?;
        stdout_writer.write_all(&output)?;
        stdout_writer.flush()?;

        let stderr_handle = std::io::stderr();
        let mut stderr_writer = stderr_handle.lock();
        stderr_writer.write_all(b"BOUNDARY_STDERR_START")?;
        stderr_writer.write_all(&error)?;
        stderr_writer.flush()?;
        Ok(())
    }

    fn write_delayed_markers() -> Result<()> {
        let root = std::env::var_os(CHILD_MARKER_ROOT_ENV)
            .context("locate the marker root for the child process test")?;
        let directory = Dir::open_ambient_dir(root, ambient_authority())?;
        directory.write("started", b"started")?;
        thread::sleep(CHILD_COMPLETION_DELAY);
        directory.write("completed", b"completed")?;
        Ok(())
    }
}
