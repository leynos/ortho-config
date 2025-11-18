//! Process execution helpers for running the `hello_world` binary in scenarios.
use super::{CommandResult, Harness};
use anyhow::{anyhow, Context, Result};
use shlex::split;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

impl Harness {
    pub(crate) fn run_hello(&mut self, args: Option<String>) -> Result<()> {
        let parsed = args
            .map(|raw| tokenise_shell_args(&raw))
            .transpose()? // Option<Result<_>> -> Result<Option<_>>
            .unwrap_or_default();
        self.run_example(parsed)
    }

    pub(crate) fn run_example(&mut self, args: Vec<String>) -> Result<()> {
        let binary = self.binary();
        let mut command = Command::new(binary.as_std_path());
        command.current_dir(self.workdir.path());
        command.args(&args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.stdin(Stdio::null());
        self.configure_environment(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("spawn {binary} binary"))?;
        let stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("capture hello_world stdout pipe"))?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("capture hello_world stderr pipe"))?;

        let stdout_reader = spawn_pipe_reader(stdout_pipe, "hello_world stdout");
        let stderr_reader = spawn_pipe_reader(stderr_pipe, "hello_world stderr");

        let status = match wait_with_timeout(&mut child, self.command_timeout()) {
            Ok(status) => status,
            Err(err) => {
                let _ = join_reader(stdout_reader, "hello_world stdout");
                let _ = join_reader(stderr_reader, "hello_world stderr");
                return Err(err);
            }
        };
        let stdout = join_reader(stdout_reader, "hello_world stdout")?;
        let stderr = join_reader(stderr_reader, "hello_world stderr")?;
        let output = Output {
            status,
            stdout,
            stderr,
        };
        self.result = Some(CommandResult::from_execution(
            output,
            binary.to_string(),
            args,
        ));
        Ok(())
    }

    pub(crate) fn result(&self) -> Result<&CommandResult> {
        self.result
            .as_ref()
            .ok_or_else(|| anyhow!("command execution result unavailable"))
    }
}

fn tokenise_shell_args(raw: &str) -> Result<Vec<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    split(trimmed).ok_or_else(|| {
        anyhow!("failed to tokenise shell arguments; raw={raw:?}, trimmed={trimmed:?}")
    })
}

fn spawn_pipe_reader(
    mut pipe: impl Read + Send + 'static,
    context_label: &'static str,
) -> thread::JoinHandle<Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut buffer = Vec::new();
        pipe.read_to_end(&mut buffer)
            .with_context(|| format!("read {context_label}"))?;
        Ok(buffer)
    })
}

fn join_reader(
    handle: thread::JoinHandle<Result<Vec<u8>>>,
    label: &'static str,
) -> Result<Vec<u8>> {
    handle
        .join()
        .map_err(|_| anyhow!("{label} reader thread panicked"))?
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Result<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .context("poll hello_world binary status")?
        {
            return Ok(status);
        }

        if start.elapsed() > timeout {
            return handle_timeout(child);
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn handle_timeout(child: &mut Child) -> Result<ExitStatus> {
    if let Some(status) = child
        .try_wait()
        .context("poll hello_world binary status after timeout")?
    {
        return Ok(status);
    }
    child
        .kill()
        .context("kill stalled hello_world binary")?;
    child
        .wait()
        .context("wait for killed hello_world binary")?;
    Err(anyhow!("hello_world binary timed out"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use camino::Utf8PathBuf;
    use std::process::Stdio;
    use tempfile::TempDir;

    struct CompiledBinary {
        _dir: TempDir,
        path: Utf8PathBuf,
    }

    impl CompiledBinary {
        fn new(source: &str) -> Self {
            let dir = TempDir::new().expect("create test binary dir");
            let src_path = dir.path().join("main.rs");
            std::fs::write(&src_path, source).expect("write test binary source");
            let bin_path = dir.path().join("bin");
            let status = Command::new("rustc")
                .arg("--edition=2021")
                .arg(&src_path)
                .arg("-o")
                .arg(&bin_path)
                .status()
                .expect("compile test binary");
            assert!(status.success(), "rustc must compile helper binary");
            let utf8_path = Utf8PathBuf::from_path_buf(bin_path).expect("utf8 test binary");
            Self {
                _dir: dir,
                path: utf8_path,
            }
        }

        fn path(&self) -> Utf8PathBuf {
            self.path.clone()
        }
    }

    fn rustc_available() -> bool {
        Command::new("rustc")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn ensure_rustc_available() -> Result<bool> {
        if rustc_available() {
            Ok(true)
        } else {
            eprintln!("skipping hello_world harness process tests: rustc not available");
            Ok(false)
        }
    }

    #[test]
    fn run_example_times_out() -> Result<()> {
        if !ensure_rustc_available()? {
            return Ok(());
        }
        let binary = CompiledBinary::new(
            r#"
            fn main() {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            "#,
        );
        let mut harness = Harness::for_tests()?;
        harness.set_binary_override(binary.path());
        harness.set_timeout_override(Duration::from_millis(50));
        let err = harness
            .run_example(Vec::new())
            .expect_err("long-running binary should time out");
        assert!(err.to_string().contains("timed out"));
        Ok(())
    }

    #[test]
    fn run_example_reports_spawn_errors() -> Result<()> {
        if !ensure_rustc_available()? {
            return Ok(());
        }
        let mut harness = Harness::for_tests()?;
        harness.set_binary_override(Utf8PathBuf::from("/definitely/missing/binary"));
        let err = harness
            .run_example(Vec::new())
            .expect_err("missing binary must error");
        assert!(err.to_string().contains("spawn"));
        Ok(())
    }

    #[test]
    fn run_example_captures_failure_status() -> Result<()> {
        if !ensure_rustc_available()? {
            return Ok(());
        }
        let binary = CompiledBinary::new(
            r#"
            fn main() {
                eprintln!("forced failure");
                std::process::exit(42);
            }
            "#,
        );
        let mut harness = Harness::for_tests()?;
        harness.set_binary_override(binary.path());
        harness.run_example(Vec::new())?;
        harness.assert_failure()?;
        harness.assert_stderr_contains("forced failure")
    }
}
