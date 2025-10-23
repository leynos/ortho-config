//! Cucumber test harness for the `hello_world` example.
use std::fs;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
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
#[world(init = Self::init)]
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

impl World {
    fn new() -> Result<Self> {
        let workdir = tempfile::tempdir().context("create hello_world workdir")?;
        Ok(Self {
            result: None,
            workdir,
            env: BTreeMap::new(),
            declarative_globals: None,
        })
    }

    async fn init() -> Result<Self> {
        async { Self::new() }.await
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Result<Self> {
        Self::new()
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

    fn declarative_globals(&self) -> Result<&GlobalArgs> {
        self.declarative_globals
            .as_ref()
            .ok_or_else(|| anyhow!("declarative globals composed before assertion"))
    }

    /// Asserts that the composed globals expose the expected recipient.
    ///
    /// # Errors
    ///
    /// Returns an error when the stored recipient does not match `expected`.
    pub fn assert_declarative_recipient<S>(&self, expected: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let globals = self.declarative_globals()?;
        let recipient = globals.recipient.as_deref().unwrap_or("");
        ensure!(
            recipient == expected.as_ref(),
            "unexpected recipient {recipient:?}"
        );
        Ok(())
    }

    /// Asserts that the composed globals expose the expected salutations.
    ///
    /// # Errors
    ///
    /// Returns an error when the stored salutations differ from `expected`.
    pub fn assert_declarative_salutations(&self, expected: &[String]) -> Result<()> {
        let globals = self.declarative_globals()?;
        ensure!(
            globals.salutations == expected,
            "unexpected salutations: {:?}",
            globals.salutations
        );
        Ok(())
    }

    /// Writes a configuration file into the scenario work directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary working directory cannot be opened or the
    /// config file cannot be written.
    pub fn write_config(&self, contents: &str) -> Result<()> {
        let dir = self
            .scenario_dir()
            .context("open hello_world workdir for config write")?;
        dir.write(CONFIG_FILE, contents)
            .with_context(|| format!("write {CONFIG_FILE}"))?;
        Ok(())
    }

    /// Writes an arbitrary configuration file into the scenario directory.
    ///
    /// # Errors
    ///
    /// Returns an error when `name` is not a simple filename or the write fails.
    pub fn write_named_file<S>(&self, name: S, contents: &str) -> Result<()>
    where
        S: AsRef<str>,
    {
        let name_ref = name.as_ref();
        ensure!(
            is_simple_filename(name_ref),
            "custom config filename must not contain path separators: {name_ref}"
        );
        let dir = self
            .scenario_dir()
            .context("open hello_world workdir for named config write")?;
        dir.write(name_ref, contents)
            .with_context(|| format!("write hello_world named config {name_ref}"))?;
        Ok(())
    }

    /// Writes the provided contents to `hello_world.toml` under an XDG config home.
    ///
    /// # Errors
    ///
    /// Returns an error if the XDG directory cannot be prepared or the file write fails.
    pub fn write_xdg_config_home(&mut self, contents: &str) -> Result<()> {
        let base = self.workdir.path().join("xdg-config");
        let config_dir = base.join("hello_world");
        fs::create_dir_all(&config_dir).context("create XDG hello_world directory")?;
        fs::write(config_dir.join("hello_world.toml"), contents)
            .context("write XDG hello_world config")?;
        let value = base.to_string_lossy().into_owned();
        self.set_env("XDG_CONFIG_HOME", value);
        Ok(())
    }

    /// Copies a repository sample configuration into the scenario directory.
    ///
    /// # Errors
    ///
    /// Returns an error if writing the sample configuration fails.
    pub fn write_sample_config<S>(&self, sample: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        self.try_write_sample_config(sample)
            .map_err(|err| anyhow!(err))?;
        Ok(())
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
        let scenario = self
            .scenario_dir()
            .map_err(|source| SampleConfigError::WriteSample {
                name: params.target_name.to_owned(),
                source,
            })?;
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

    fn scenario_dir(&self) -> std::io::Result<cap_std::fs::Dir> {
        cap_std::fs::Dir::open_ambient_dir(self.workdir.path(), cap_std::ambient_authority())
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
    /// # Errors
    ///
    /// Returns an error if argument tokenisation fails or the underlying command cannot
    /// be executed successfully.
    pub async fn run_hello(&mut self, args: Option<String>) -> Result<()> {
        let parsed = if let Some(raw) = args {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                split(trimmed).ok_or_else(|| anyhow!("parse CLI arguments"))?
            }
        } else {
            Vec::new()
        };
        self.run_example(parsed).await
    }

    /// Runs the example binary with the provided arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning the example binary fails or if the process does not exit
    /// within [`COMMAND_TIMEOUT`].
    pub async fn run_example(&mut self, args: Vec<String>) -> Result<()> {
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

    /// Returns the captured command output from the most recent run.
    ///
    /// # Errors
    ///
    /// Returns an error if the example has not been executed in the current scenario.
    pub fn result(&self) -> Result<&CommandResult> {
        self.result
            .as_ref()
            .ok_or_else(|| anyhow!("command execution result unavailable"))
    }

    fn with_result<T, F>(&self, action: F) -> Result<T>
    where
        F: FnOnce(&CommandResult) -> Result<T>,
    {
        let result = self.result()?;
        action(result)
    }

    /// Asserts that the most recent command succeeded.
    ///
    /// # Errors
    ///
    /// Returns an error if the last invocation exited with a non-zero status.
    pub fn assert_success(&self) -> Result<()> {
        self.with_result(|result| {
            ensure!(
                result.success,
                "expected success; status: {:?}; cmd: {} {:?}; stderr: {}",
                result.status,
                result.binary,
                result.args,
                result.stderr
            );
            Ok(())
        })
    }

    /// Asserts that the most recent command failed.
    ///
    /// # Errors
    ///
    /// Returns an error if the last invocation exited successfully.
    pub fn assert_failure(&self) -> Result<()> {
        self.with_result(|result| {
            ensure!(
                !result.success,
                "expected failure; status: {:?}; cmd: {} {:?}; stdout: {}",
                result.status,
                result.binary,
                result.args,
                result.stdout
            );
            Ok(())
        })
    }

    /// Asserts that stdout contains the expected substring.
    ///
    /// # Errors
    ///
    /// Returns an error if stdout does not include the provided fragment.
    pub fn assert_stdout_contains<S>(&self, expected: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let expected_text = expected.as_ref();
        self.with_result(|result| {
            ensure!(
                result.stdout.contains(expected_text),
                "stdout did not contain {expected_text:?}; status: {:?}; cmd: {} {:?}; stdout was: {:?}",
                result.status,
                result.binary,
                result.args,
                result.stdout
            );
            Ok(())
        })
    }

    /// Asserts that stderr contains the expected substring.
    ///
    /// # Errors
    ///
    /// Returns an error if stderr does not include the provided fragment.
    pub fn assert_stderr_contains<S>(&self, expected: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let expected_text = expected.as_ref();
        self.with_result(|result| {
            ensure!(
                result.stderr.contains(expected_text),
                "stderr did not contain {expected_text:?}; status: {:?}; cmd: {} {:?}; stderr was: {:?}",
                result.status,
                result.binary,
                result.args,
                result.stderr
            );
            Ok(())
        })
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
    use anyhow::{Result, anyhow, ensure};
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
    ) -> Result<()> {
        let world = super::World::for_tests()?;
        let Err(error) = world.try_write_sample_config(sample) else {
            return Err(anyhow!("sample config copy should fail"));
        };

        match (expected, error) {
            ("missing", super::SampleConfigError::OpenSample { name, .. })
            | ("invalid", super::SampleConfigError::InvalidName { name }) => {
                ensure!(name == sample, "unexpected sample name: {name}");
            }
            (_, other) => return Err(anyhow!("unexpected sample config error: {other:?}")),
        }
        Ok(())
    }

    #[rstest]
    fn copy_sample_config_writes_all_files() -> Result<()> {
        let world = super::World::for_tests()?;
        let tempdir = tempfile::tempdir().context("create sample source")?;
        let source =
            cap_std::fs::Dir::open_ambient_dir(tempdir.path(), cap_std::ambient_authority())
                .context("open sample source dir")?;
        source
            .write("overrides.toml", r#"extends = ["baseline.toml"]"#)
            .context("write overrides sample")?;
        source
            .write("baseline.toml", "")
            .context("write baseline sample")?;

        let mut visited = std::collections::BTreeSet::new();
        let params = super::ConfigCopyParams {
            source: &source,
            source_name: "overrides.toml",
            target_name: ".hello_world.toml",
        };
        world
            .copy_sample_config(params, &mut visited)
            .map_err(|err| anyhow!(err))?;

        let scenario = world
            .scenario_dir()
            .context("open hello_world scenario dir")?;
        let overrides = scenario
            .read_to_string(".hello_world.toml")
            .context("read copied overrides")?;
        ensure!(overrides.contains("baseline.toml"));
        let baseline = scenario
            .read_to_string("baseline.toml")
            .context("read copied baseline")?;
        ensure!(baseline.is_empty(), "expected empty baseline");
        Ok(())
    }

    #[rstest]
    fn copy_sample_config_deduplicates_repeated_extends() -> Result<()> {
        let world = super::World::for_tests()?;
        let tempdir = tempfile::tempdir().context("create sample source")?;
        let source =
            cap_std::fs::Dir::open_ambient_dir(tempdir.path(), cap_std::ambient_authority())
                .context("open sample source dir")?;
        source
            .write(
                "overrides.toml",
                r#"extends = ["baseline.toml", "baseline.toml"]"#,
            )
            .context("write overrides sample")?;
        source
            .write("baseline.toml", "")
            .context("write baseline sample")?;

        let mut visited = std::collections::BTreeSet::new();
        let params = super::ConfigCopyParams {
            source: &source,
            source_name: "overrides.toml",
            target_name: ".hello_world.toml",
        };
        world
            .copy_sample_config(params, &mut visited)
            .map_err(|err| anyhow!(err))?;

        let visited: Vec<_> = visited.into_iter().collect();
        ensure!(
            visited == vec!["baseline.toml".to_owned(), "overrides.toml".to_owned()],
            "unexpected visited list: {:?}",
            visited
        );
        Ok(())
    }
}
