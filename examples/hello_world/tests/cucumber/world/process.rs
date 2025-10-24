//! Process execution helpers for running the `hello_world` binary in scenarios.
use super::{COMMAND_TIMEOUT, CommandResult, World, binary_path};
use anyhow::{Context, Result, anyhow};
use shlex::split;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

impl World {
    pub(crate) async fn run_hello(&mut self, args: Option<String>) -> Result<()> {
        let parsed = if let Some(raw) = args {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                split(trimmed)
                    .ok_or_else(|| anyhow!("failed to tokenise shell arguments: {trimmed:?}"))?
            }
        } else {
            Vec::new()
        };
        self.run_example(parsed).await
    }

    pub(crate) async fn run_example(&mut self, args: Vec<String>) -> Result<()> {
        let binary = binary_path();
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

        let wait_future = async move {
            if let Ok(status) = timeout(COMMAND_TIMEOUT, child.wait()).await {
                status.context("wait for hello_world binary")
            } else {
                child
                    .kill()
                    .await
                    .context("kill stalled hello_world binary")?;
                child
                    .wait()
                    .await
                    .context("wait for killed hello_world binary")?;
                Err(anyhow!("hello_world binary timed out"))
            }
        };

        let stdout_future = async move {
            let mut buffer = Vec::new();
            let mut pipe = stdout_pipe;
            pipe.read_to_end(&mut buffer)
                .await
                .context("read hello_world stdout")?;
            Ok(buffer)
        };

        let stderr_future = async move {
            let mut buffer = Vec::new();
            let mut pipe = stderr_pipe;
            pipe.read_to_end(&mut buffer)
                .await
                .context("read hello_world stderr")?;
            Ok(buffer)
        };

        let (status, stdout, stderr) = tokio::try_join!(wait_future, stdout_future, stderr_future)?;
        let output = std::process::Output {
            status,
            stdout,
            stderr,
        };
        let mut result = CommandResult::from(output);
        binary.as_str().clone_into(&mut result.binary);
        result.args = args;
        self.result = Some(result);
        Ok(())
    }

    pub(crate) fn result(&self) -> Result<&CommandResult> {
        self.result
            .as_ref()
            .ok_or_else(|| anyhow!("command execution result unavailable"))
    }
}
