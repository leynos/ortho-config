//! Harness state and command helpers for the `rstest-bdd` harness.
//!
//! The harness isolates environment variables per scenario: values stored in
//! `env` are applied only when launching the command and never leak between
//! runs. Each scenario owns its own temporary working directory (`workdir`),
//! which is removed automatically when the harness is dropped. Command results
//! and declaratively composed globals live only for the lifetime of a single
//! scenario to keep assertions deterministic.

mod assertions;
mod env;
mod process;
mod samples;

#[cfg(test)]
mod tests;

pub(crate) use super::config;
pub(crate) use super::config::SampleConfigError;
pub(crate) use super::{CONFIG_FILE, ENV_PREFIX};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cap_std::fs::Dir;
use hello_world::cli::GlobalArgs;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::time::Duration;
use tempfile::TempDir;

/// Shared state threaded through behavioural steps.
#[derive(Debug)]
pub struct Harness {
    /// Result captured after invoking the binary.
    result: Option<CommandResult>,
    /// Temporary working directory isolated per scenario.
    workdir: TempDir,
    /// Environment variables to inject when running the binary.
    env: BTreeMap<String, String>,
    /// Declaratively composed globals used by behavioural tests.
    declarative_globals: Option<GlobalArgs>,
    /// Optional binary override used by targeted tests.
    binary_override: Option<Utf8PathBuf>,
    /// Optional timeout override (primarily for targeted tests).
    timeout_override: Option<Duration>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Expect<'a> {
    Success,
    Failure,
    StdoutContains(&'a str),
    StderrContains(&'a str),
}

impl Harness {
    pub(crate) fn new() -> Result<Self> {
        let workdir = TempDir::new().context("create hello_world workdir")?;
        Ok(Self {
            result: None,
            workdir,
            env: BTreeMap::new(),
            declarative_globals: None,
            binary_override: None,
            timeout_override: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Result<Self> {
        Self::new()
    }

    fn scenario_dir(&self) -> std::io::Result<Dir> {
        Dir::open_ambient_dir(self.workdir.path(), cap_std::ambient_authority())
    }

    pub(crate) fn binary(&self) -> Utf8PathBuf {
        self.binary_override
            .clone()
            .unwrap_or_else(super::binary_path)
    }

    pub(crate) fn command_timeout(&self) -> Duration {
        self.timeout_override.unwrap_or(super::COMMAND_TIMEOUT)
    }

    #[cfg(test)]
    pub(crate) fn set_binary_override<P>(&mut self, path: P)
    where
        P: Into<Utf8PathBuf>,
    {
        self.binary_override = Some(path.into());
    }

    #[cfg(test)]
    pub(crate) const fn set_timeout_override(&mut self, duration: Duration) {
        self.timeout_override = Some(duration);
    }
}

/// Output captured from executing the CLI.
#[derive(Debug, Default)]
pub(crate) struct CommandResult {
    status: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
    binary: String,
    args: Vec<String>,
}

impl CommandResult {
    /// Formats common command execution context for error messages.
    fn command_context(&self) -> String {
        format!(
            "status: {:?}; cmd: {} {:?}",
            self.status, self.binary, self.args
        )
    }

    pub(crate) fn from_execution(
        output: std::process::Output,
        binary: String,
        args: Vec<String>,
    ) -> Self {
        let std::process::Output {
            status,
            stdout,
            stderr,
        } = output;
        let normalised_stdout = normalise_newlines(String::from_utf8_lossy(&stdout));
        let normalised_stderr = normalise_newlines(String::from_utf8_lossy(&stderr));

        Self {
            status: status.code(),
            success: status.success(),
            stdout: normalised_stdout,
            stderr: normalised_stderr,
            binary,
            args,
        }
    }
}

/// Converts Windows newlines to their `Unix` equivalent so substring assertions
/// behave consistently across platforms. We normalise both CRLF and bare CR
/// sequences because `PowerShell` occasionally emits the latter when piping
/// output between commands.
fn normalise_newlines(text: Cow<'_, str>) -> String {
    if !text.contains('\r') {
        return text.into_owned();
    }
    let mut normalised = text.into_owned();
    normalised = normalised.replace("\r\n", "\n");
    if normalised.contains('\r') {
        normalised = normalised.replace('\r', "\n");
    }
    normalised
}
