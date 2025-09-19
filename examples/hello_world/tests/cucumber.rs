//! Cucumber test harness for the `hello_world` example.
use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::World as _;
use shlex::split;
use tokio::process::Command;
use tokio::time::timeout;

mod steps;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared state threaded through Cucumber steps.
#[derive(Debug, Default, cucumber::World)]
pub struct World {
    /// Result captured after invoking the binary.
    pub result: Option<CommandResult>,
}

/// Output captured from executing the CLI.
#[derive(Debug, Default)]
pub struct CommandResult {
    pub status: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl From<std::process::Output> for CommandResult {
    fn from(output: std::process::Output) -> Self {
        Self {
            status: output.status.code(),
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

impl World {
    /// Runs the `hello_world` binary with optional CLI arguments.
    ///
    /// # Panics
    ///
    /// Panics if argument tokenisation fails or if the underlying command
    /// fails to execute successfully.
    pub async fn run_hello(&mut self, args: Option<&str>) {
        let parsed = match args {
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    Vec::new()
                } else {
                    split(trimmed).expect("parse CLI arguments")
                }
            }
            None => Vec::new(),
        };
        self.run_example(parsed).await;
    }

    /// Runs the example binary with the provided arguments.
    ///
    /// # Panics
    ///
    /// Panics if spawning the example binary fails or if the process
    /// does not exit within [`COMMAND_TIMEOUT`].
    pub async fn run_example(&mut self, args: Vec<String>) {
        let binary = binary_path();
        let mut command = Command::new(binary.as_std_path());
        command.args(&args);
        let output = timeout(COMMAND_TIMEOUT, command.output())
            .await
            .expect("hello_world binary timed out")
            .expect("execute hello_world binary");
        self.result = Some(CommandResult::from(output));
    }

    /// Returns the captured command output from the most recent run.
    ///
    /// # Panics
    ///
    /// Panics if the example has not been executed in the current scenario.
    #[must_use]
    pub fn result(&self) -> &CommandResult {
        self.result
            .as_ref()
            .expect("command execution result available")
    }

    /// Asserts that the most recent command succeeded.
    ///
    /// # Panics
    ///
    /// Panics if the last invocation exited with a non-zero status.
    pub fn assert_success(&self) {
        let result = self.result();
        assert!(
            result.success,
            "expected success, stderr was: {}",
            result.stderr
        );
    }

    /// Asserts that the most recent command failed.
    ///
    /// # Panics
    ///
    /// Panics if the last invocation exited successfully.
    pub fn assert_failure(&self) {
        let result = self.result();
        assert!(
            !result.success,
            "expected failure, stdout was: {}",
            result.stdout
        );
    }

    /// Asserts that stdout contains the expected substring.
    ///
    /// # Panics
    ///
    /// Panics if stdout does not include the provided fragment.
    pub fn assert_stdout_contains(&self, expected: &str) {
        let result = self.result();
        assert!(
            result.stdout.contains(expected),
            "stdout did not contain {expected:?}. stdout was: {:?}",
            result.stdout
        );
    }

    /// Asserts that stderr contains the expected substring.
    ///
    /// # Panics
    ///
    /// Panics if stderr does not include the provided fragment.
    pub fn assert_stderr_contains(&self, expected: &str) {
        let result = self.result();
        assert!(
            result.stderr.contains(expected),
            "stderr did not contain {expected:?}. stderr was: {:?}",
            result.stderr
        );
    }
}

fn binary_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!(
        "CARGO_BIN_EXE_hello_world",
        "Cargo must set the hello_world binary path for integration tests",
    ))
}

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
