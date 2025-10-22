//! Cucumber test harness for the `hello_world` example.
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use std::fs;
use std::process::Stdio;
use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::World as _;
use hello_world::cli::GlobalArgs;
use shlex::split;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
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
    /// Declaratively composed globals used by behavioural tests.
    declarative_globals: Option<GlobalArgs>,
}

impl Default for World {
    fn default() -> Self {
        let workdir = tempfile::tempdir().expect("create hello_world workdir");
        Self {
            result: None,
            workdir,
            env: BTreeMap::new(),
            declarative_globals: None,
        }
    }
}

/// Output captured from executing the CLI.
#[derive(Debug, Default)]
pub struct CommandResult {
    /// Exit status reported by the subprocess, when available.
    pub status: Option<i32>,
    /// Convenience flag reflecting whether the subprocess succeeded.
    pub success: bool,
    /// Captured standard output emitted by the subprocess.
    pub stdout: String,
    /// Captured standard error emitted by the subprocess.
    pub stderr: String,
    /// Path to the binary that was executed for the scenario.
    pub binary: String,
    /// Arguments used when launching the binary.
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

/// Errors raised while preparing fixture configuration for scenarios.
#[derive(Debug, Error)]
pub enum SampleConfigError {
    /// Failed to open the directory containing sample configurations.
    #[error("failed to open hello world sample config directory: {path}")]
    OpenConfigDir {
        /// Path to the directory that could not be opened.
        path: String,
        /// Underlying IO error reported by the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Encountered an invalid file name when selecting a sample.
    #[error("invalid hello world sample config name {name}")]
    InvalidName {
        /// Name that violated the sample selection rules.
        name: String,
    },
    /// Failed to open an individual sample configuration file.
    #[error("failed to open hello world sample config {name}")]
    OpenSample {
        /// Name of the sample configuration involved in the failure.
        name: String,
        /// Source error raised while opening the sample.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read from an opened sample configuration file.
    #[error("failed to read hello world sample config {name}")]
    ReadSample {
        /// Name of the sample configuration being read.
        name: String,
        /// Source error raised during file read.
        #[source]
        source: std::io::Error,
    },
    /// Failed to write a sample configuration into the scenario directory.
    #[error("failed to write hello world sample config {name}")]
    WriteSample {
        /// Name of the sample configuration that could not be written.
        name: String,
        /// Source error raised during file write.
        #[source]
        source: std::io::Error,
    },
}

fn is_simple_filename(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(std::path::is_separator)
}

fn ensure_simple_filename(name: &str) -> Result<(), SampleConfigError> {
    if is_simple_filename(name) {
        Ok(())
    } else {
        Err(SampleConfigError::InvalidName {
            name: name.to_owned(),
        })
    }
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
    pub fn remove_env<S>(&mut self, key: S)
    where
        S: AsRef<str>,
    {
        self.env.remove(key.as_ref());
    }

    /// Stores declaratively merged globals for later assertions.
    pub fn set_declarative_globals(&mut self, globals: GlobalArgs) {
        self.declarative_globals = Some(globals);
    }

    fn declarative_globals(&self) -> &GlobalArgs {
        self.declarative_globals
            .as_ref()
            .expect("declarative globals composed before assertion")
    }

    /// Asserts that the composed globals expose the expected recipient.
    ///
    /// # Panics
    ///
    /// Panics when the stored recipient does not match `expected`.
    pub fn assert_declarative_recipient<S>(&self, expected: S)
    where
        S: AsRef<str>,
    {
        let globals = self.declarative_globals();
        let recipient = globals.recipient.as_deref().unwrap_or("");
        assert_eq!(recipient, expected.as_ref());
    }

    /// Asserts that the composed globals expose the expected salutations.
    ///
    /// # Panics
    ///
    /// Panics when the stored salutations differ from `expected`.
    pub fn assert_declarative_salutations(&self, expected: &[String]) {
        let globals = self.declarative_globals();
        assert_eq!(globals.salutations, expected);
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

    /// Writes an arbitrary configuration file into the scenario directory.
    ///
    /// # Panics
    ///
    /// Panics when `name` is not a simple filename or when writing fails.
    pub fn write_named_file<S>(&self, name: S, contents: &str)
    where
        S: AsRef<str>,
    {
        let name_ref = name.as_ref();
        assert!(
            is_simple_filename(name_ref),
            "custom config filename must not contain path separators: {name_ref}"
        );
        let dir = self.scenario_dir();
        dir.write(name_ref, contents)
            .expect("write hello_world named config");
    }

    /// Writes the provided contents to `hello_world.toml` under an XDG config home.
    ///
    /// # Panics
    ///
    /// Panics if the XDG directory cannot be prepared or the file write fails.
    pub fn write_xdg_config_home(&mut self, contents: &str) {
        let base = self.workdir.path().join("xdg-config");
        let config_dir = base.join("hello_world");
        fs::create_dir_all(&config_dir).expect("create XDG hello_world directory");
        fs::write(config_dir.join("hello_world.toml"), contents)
            .expect("write XDG hello_world config");
        let value = base.to_string_lossy().into_owned();
        self.set_env("XDG_CONFIG_HOME", value);
    }

    /// Copies a repository sample configuration into the scenario directory.
    ///
    /// # Panics
    ///
    /// Panics if writing the sample configuration fails.
    pub fn write_sample_config<S>(&self, sample: S)
    where
        S: AsRef<str>,
    {
        self.try_write_sample_config(sample)
            .expect("write hello_world sample config");
    }

    /// Attempts to copy a repository sample configuration into the scenario directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the sample or any extended configuration cannot be read
    /// or when writing into the scenario directory fails.
    pub fn try_write_sample_config<S>(&self, sample: S) -> Result<(), SampleConfigError>
    where
        S: AsRef<str>,
    {
        let sample_name = sample.as_ref();
        ensure_simple_filename(sample_name)?;
        let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_dir = manifest_dir.join("config");
        let source = cap_std::fs::Dir::open_ambient_dir(
            config_dir.as_std_path(),
            cap_std::ambient_authority(),
        )
        .map_err(|source| SampleConfigError::OpenConfigDir {
            path: config_dir.as_str().to_owned(),
            source,
        })?;
        let mut visited = BTreeSet::new();
        let params = ConfigCopyParams {
            source: &source,
            source_name: sample_name,
            target_name: CONFIG_FILE,
        };
        self.copy_sample_config(params, &mut visited)?;
        Ok(())
    }

    fn copy_sample_config(
        &self,
        params: ConfigCopyParams<'_>,
        visited: &mut BTreeSet<String>,
    ) -> Result<(), SampleConfigError> {
        ensure_simple_filename(params.source_name)?;
        ensure_simple_filename(params.target_name)?;
        if !visited.insert(params.source_name.to_owned()) {
            return Ok(());
        }
        let contents = params
            .source
            .read_to_string(params.source_name)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::NotFound {
                    SampleConfigError::OpenSample {
                        name: params.source_name.to_owned(),
                        source,
                    }
                } else {
                    SampleConfigError::ReadSample {
                        name: params.source_name.to_owned(),
                        source,
                    }
                }
            })?;
        let scenario = self.scenario_dir();
        scenario
            .write(params.target_name, &contents)
            .map_err(|source| SampleConfigError::WriteSample {
                name: params.target_name.to_owned(),
                source,
            })?;
        for base in parse_extends(&contents) {
            let base_name = base.as_str();
            ensure_simple_filename(base_name)?;
            let base_params = ConfigCopyParams {
                source: params.source,
                source_name: base_name,
                target_name: base_name,
            };
            self.copy_sample_config(base_params, visited)?;
        }
        Ok(())
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
    pub async fn run_hello(&mut self, args: Option<String>) {
        let parsed = args.map_or_else(Vec::new, |raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                split(trimmed).expect("parse CLI arguments")
            }
        });
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
                    child.kill().await.expect("kill stalled hello_world binary");
                    child
                        .wait()
                        .await
                        .expect("wait for killed hello_world binary");
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
        binary.as_str().clone_into(&mut result.binary);
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
        let expected_text = expected.as_ref();
        let result = self.result();
        assert!(
            result.stdout.contains(expected_text),
            "stdout did not contain {expected_text:?}; status: {:?}; cmd: {} {:?}; stdout was: {:?}",
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
        let expected_text = expected.as_ref();
        let result = self.result();
        assert!(
            result.stderr.contains(expected_text),
            "stderr did not contain {expected_text:?}; status: {:?}; cmd: {} {:?}; stderr was: {:?}",
            result.status,
            result.binary,
            result.args,
            result.stderr
        );
    }
}

fn parse_extends(contents: &str) -> Vec<String> {
    let document: toml::Value = match toml::from_str(contents) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    match document.get("extends") {
        Some(toml::Value::String(path)) => extract_single_path(path),
        Some(toml::Value::Array(values)) => extract_multiple_paths(values),
        _ => Vec::new(),
    }
}

fn extract_single_path(path: &str) -> Vec<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_owned()]
    }
}

fn extract_multiple_paths(values: &[toml::Value]) -> Vec<String> {
    values.iter().filter_map(extract_string_value).collect()
}

fn extract_string_value(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(path) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        _ => None,
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    #[test]
    fn parse_extends_single_string() {
        let out = super::parse_extends(r#"extends = "base.toml""#);
        assert_eq!(out, vec!["base.toml"]);
    }

    #[test]
    fn parse_extends_array_mixed_types_filters_non_strings() {
        let out =
            super::parse_extends(r#"extends = ["a.toml", 42, " b . toml ", "", { k = "v" }]"#);
        assert_eq!(out, vec!["a.toml", "b . toml"]);
    }

    #[test]
    fn parse_extends_ignores_malformed_toml() {
        let out = super::parse_extends(r#"extends = [ "a.toml", ""#);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_extends_nested_array_filters_deeper_levels() {
        let out = super::parse_extends(r#"extends = [["base.toml"], " extra.toml ", ""]"#);
        assert_eq!(out, vec!["extra.toml"]);
    }

    #[rstest]
    #[case("nonexistent.toml", "missing")]
    #[case("../invalid.toml", "invalid")]
    fn try_write_sample_config_reports_expected_errors(
        #[case] sample: &str,
        #[case] expected: &str,
    ) {
        let world = super::World::default();
        let error = world
            .try_write_sample_config(sample)
            .expect_err("sample config copy should fail");

        match (expected, error) {
            ("missing", super::SampleConfigError::OpenSample { name, .. })
            | ("invalid", super::SampleConfigError::InvalidName { name }) => {
                assert_eq!(name, sample);
            }
            (_, other) => panic!("unexpected sample config error: {other:?}"),
        }
    }

    #[rstest]
    fn copy_sample_config_writes_all_files() {
        let world = super::World::default();
        let tempdir = tempfile::tempdir().expect("create sample source");
        let source =
            cap_std::fs::Dir::open_ambient_dir(tempdir.path(), cap_std::ambient_authority())
                .expect("open sample source dir");
        source
            .write("overrides.toml", r#"extends = ["baseline.toml"]"#)
            .expect("write overrides sample");
        source
            .write("baseline.toml", "")
            .expect("write baseline sample");

        let mut visited = std::collections::BTreeSet::new();
        let params = super::ConfigCopyParams {
            source: &source,
            source_name: "overrides.toml",
            target_name: ".hello_world.toml",
        };
        world
            .copy_sample_config(params, &mut visited)
            .expect("copy sample config");

        let scenario = world.scenario_dir();
        let overrides = scenario
            .read_to_string(".hello_world.toml")
            .expect("read copied overrides");
        assert!(overrides.contains("baseline.toml"));
        let baseline = scenario
            .read_to_string("baseline.toml")
            .expect("read copied baseline");
        assert!(baseline.is_empty());
    }

    #[rstest]
    fn copy_sample_config_deduplicates_repeated_extends() {
        let world = super::World::default();
        let tempdir = tempfile::tempdir().expect("create sample source");
        let source =
            cap_std::fs::Dir::open_ambient_dir(tempdir.path(), cap_std::ambient_authority())
                .expect("open sample source dir");
        source
            .write(
                "overrides.toml",
                r#"extends = ["baseline.toml", "baseline.toml"]"#,
            )
            .expect("write overrides sample");
        source
            .write("baseline.toml", "")
            .expect("write baseline sample");

        let mut visited = std::collections::BTreeSet::new();
        let params = super::ConfigCopyParams {
            source: &source,
            source_name: "overrides.toml",
            target_name: ".hello_world.toml",
        };
        world
            .copy_sample_config(params, &mut visited)
            .expect("copy sample config");

        let visited: Vec<_> = visited.into_iter().collect();
        assert_eq!(
            visited,
            vec!["baseline.toml".to_owned(), "overrides.toml".to_owned()]
        );
    }
}
