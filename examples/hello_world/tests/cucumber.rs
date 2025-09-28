//! Cucumber test harness for the `hello_world` example.
use std::process::Stdio;
use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::World as _;
use shlex::split;
use std::collections::{BTreeMap, BTreeSet};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

mod steps;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

const CONFIG_FILE: &str = ".hello_world.toml";
const ENV_PREFIX: &str = "HELLO_WORLD_";

/// Shared state threaded through Cucumber steps.
#[derive(Debug, cucumber::World)]
pub struct World {
    /// Result captured after invoking the binary.
    result: Option<CommandResult>,
    /// Temporary working directory isolated per scenario.
    workdir: tempfile::TempDir,
    /// Environment variables to inject when running the binary.
    env: BTreeMap<String, String>,
}

impl Default for World {
    fn default() -> Self {
        let workdir = tempfile::tempdir().expect("create hello_world workdir");
        Self {
            result: None,
            workdir,
            env: BTreeMap::new(),
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

#[derive(Debug, Copy, Clone)]
struct ConfigCopyParams<'a> {
    source: &'a cap_std::fs::Dir,
    source_name: &'a str,
    target_name: &'a str,
}

impl World {
    /// Records an environment variable override scoped to the scenario.
    pub fn set_env<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.env.insert(key.into(), value.into());
    }

    /// Removes a scenario-scoped environment variable override.
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
        dir.write(CONFIG_FILE, contents)
            .expect("write hello_world config");
    }

    /// Copies a repository sample configuration into the scenario directory.
    ///
    /// # Panics
    ///
    /// Panics if the sample configuration or any referenced base file cannot be
    /// read or written.
    pub fn write_sample_config(&self, sample: &str) {
        let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_dir = manifest_dir.join("config");
        let source = cap_std::fs::Dir::open_ambient_dir(
            config_dir.as_std_path(),
            cap_std::ambient_authority(),
        )
        .expect("open hello_world sample config directory");
        let mut visited = BTreeSet::new();
        let params = ConfigCopyParams {
            source: &source,
            source_name: sample,
            target_name: CONFIG_FILE,
        };
        self.copy_sample_config(params, &mut visited);
    }

    fn copy_sample_config(&self, params: ConfigCopyParams<'_>, visited: &mut BTreeSet<String>) {
        if !visited.insert(params.source_name.to_string()) {
            return;
        }
        let contents = params
            .source
            .read_to_string(params.source_name)
            .expect("read hello_world sample config");
        let scenario = self.scenario_dir();
        scenario
            .write(params.target_name, &contents)
            .expect("write hello_world sample config");
        if let Some(base) = parse_extends(&contents) {
            let base_params = ConfigCopyParams {
                source: params.source,
                source_name: &base,
                target_name: &base,
            };
            self.copy_sample_config(base_params, visited);
        }
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
                .is_some_and(|name| name.starts_with(ENV_PREFIX))
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
        command.stdin(Stdio::null());
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

fn parse_extends(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("extends") {
            return None;
        }
        let (_, raw) = trimmed.split_once('=')?;
        let value = raw.split('#').next().unwrap_or("").trim();
        let value = value.trim_matches(|ch| ch == '"' || ch == '\'');
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
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
