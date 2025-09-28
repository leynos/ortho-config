//! Cucumber test harness for the `hello_world` example.
use std::process::Stdio;
use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::World as _;
use shlex::split;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

mod steps;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared state threaded through Cucumber steps.
#[derive(Debug, cucumber::World)]
pub struct World {
    /// Result captured after invoking the binary.
    pub result: Option<CommandResult>,
    /// Temporary working directory isolated per scenario.
    workdir: tempfile::TempDir,
    /// Environment variables to inject when running the binary.
    env: std::collections::BTreeMap<String, String>,
}

impl Default for World {
    fn default() -> Self {
        let workdir = tempfile::tempdir().expect("create hello_world workdir");
        Self {
            result: None,
            workdir,
            env: std::collections::BTreeMap::new(),
        }
    }
}

/// Output captured from executing the CLI.
#[derive(Debug, Default)]
pub struct CommandResult {
    pub status: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub binary: String,
    pub args: Vec<String>,
}

impl From<std::process::Output> for CommandResult {
    fn from(output: std::process::Output) -> Self {
        Self {
            status: output.status.code(),
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            binary: String::new(),
            args: Vec::new(),
        }
    }
}

impl World {
    /// Records an environment variable to be injected for the next command.
    pub fn set_env(&mut self, key: String, value: String) {
        self.env.insert(key, value);
    }

    /// Removes an environment variable override for the next command.
    ///
    /// This only updates the world-scoped overrides map so the process
    /// environment remains untouched during the scenario.
    pub fn remove_env(&mut self, key: &str) {
        self.env.remove(key);
    }

    /// Writes a configuration file into the scenario work directory.
    ///
    /// # Panics
    ///
    /// Panics if the temporary working directory cannot be opened or the
    /// config file cannot be written.
    pub fn write_config(&self, contents: &str) {
        let dir = self.scenario_dir();
        dir.write(".hello-world.toml", contents)
            .expect("write hello_world config");
    }

    fn scenario_dir(&self) -> cap_std::fs::Dir {
        cap_std::fs::Dir::open_ambient_dir(self.workdir.path(), cap_std::ambient_authority())
            .expect("open hello_world workdir")
    }

    fn configure_environment(&self, command: &mut Command) {
        Self::scrub_command_environment(command);
        for (key, value) in &self.env {
            command.env(key, value);
        }
    }

    fn scrub_command_environment(command: &mut Command) {
        for (key, _) in std::env::vars_os() {
            if key
                .to_str()
                .is_some_and(|name| name.starts_with("HELLO_WORLD_"))
            {
                command.env_remove(&key);
            }
        }
    }

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
        command.current_dir(self.workdir.path());
        command.args(&args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        self.configure_environment(&mut command);
        let mut child = command.spawn().expect("spawn hello_world binary");
        let stdout_pipe = child
            .stdout
            .take()
            .expect("capture hello_world stdout pipe");
        let stderr_pipe = child
            .stderr
            .take()
            .expect("capture hello_world stderr pipe");

        let wait_future = async move {
            match timeout(COMMAND_TIMEOUT, child.wait()).await {
                Ok(Ok(status)) => status,
                Ok(Err(err)) => panic!("wait for hello_world binary: {err}"),
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    panic!("hello_world binary timed out");
                }
            }
        };

        let stdout_future = async move {
            let mut buffer = Vec::new();
            let mut pipe = stdout_pipe;
            pipe.read_to_end(&mut buffer)
                .await
                .expect("read hello_world stdout");
            buffer
        };

        let stderr_future = async move {
            let mut buffer = Vec::new();
            let mut pipe = stderr_pipe;
            pipe.read_to_end(&mut buffer)
                .await
                .expect("read hello_world stderr");
            buffer
        };

        let (status, stdout, stderr) = tokio::join!(wait_future, stdout_future, stderr_future);
        let output = std::process::Output {
            status,
            stdout,
            stderr,
        };
        let mut result = CommandResult::from(output);
        result.binary = binary.to_string();
        result.args = args;
        self.result = Some(result);
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
            "expected success; status: {:?}; cmd: {} {:?}; stderr: {}",
            result.status, result.binary, result.args, result.stderr
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
            "expected failure; status: {:?}; cmd: {} {:?}; stdout: {}",
            result.status, result.binary, result.args, result.stdout
        );
    }

    /// Asserts that stdout contains the expected substring.
    ///
    /// # Panics
    ///
    /// Panics if stdout does not include the provided fragment.
    pub fn assert_stdout_contains<S>(&self, expected: S)
    where
        S: AsRef<str>,
    {
        let expected = expected.as_ref();
        let result = self.result();
        assert!(
            result.stdout.contains(expected),
            "stdout did not contain {expected:?}; status: {:?}; cmd: {} {:?}; stdout was: {:?}",
            result.status,
            result.binary,
            result.args,
            result.stdout
        );
    }

    /// Asserts that stderr contains the expected substring.
    ///
    /// # Panics
    ///
    /// Panics if stderr does not include the provided fragment.
    pub fn assert_stderr_contains<S>(&self, expected: S)
    where
        S: AsRef<str>,
    {
        let expected = expected.as_ref();
        let result = self.result();
        assert!(
            result.stderr.contains(expected),
            "stderr did not contain {expected:?}; status: {:?}; cmd: {} {:?}; stderr was: {:?}",
            result.status,
            result.binary,
            result.args,
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
