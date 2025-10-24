//! World state and command helpers for the Cucumber harness.
//! The harness isolates environment variables per scenario: values stored in
//! `env` are applied only when launching the command and never leak between
//! runs. Each scenario owns its own temporary working directory (`workdir`),
//! which is removed automatically when the world is dropped. Command results
//! and declaratively composed globals live only for the lifetime of a single
//! scenario to keep assertions deterministic.

mod assertions;
mod env;
mod process;
mod samples;

mod tests;

pub(crate) use super::SampleConfigError;
pub(crate) use super::config;
pub(crate) use super::{COMMAND_TIMEOUT, CONFIG_FILE, ENV_PREFIX, binary_path};

use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hello_world::cli::GlobalArgs;
use std::collections::BTreeMap;
use tempfile::TempDir;

/// Shared state threaded through Cucumber steps.
#[derive(Debug, cucumber::World)]
#[world(init = Self::init)]
pub struct World {
    /// Result captured after invoking the binary.
    result: Option<CommandResult>,
    /// Temporary working directory isolated per scenario.
    workdir: TempDir,
    /// Environment variables to inject when running the binary.
    env: BTreeMap<String, String>,
    /// Declaratively composed globals used by behavioural tests.
    declarative_globals: Option<GlobalArgs>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Expect<'a> {
    Success,
    Failure,
    StdoutContains(&'a str),
    StderrContains(&'a str),
}

impl World {
    pub(crate) fn new() -> Result<Self> {
        let workdir = TempDir::new().context("create hello_world workdir")?;
        Ok(Self {
            result: None,
            workdir,
            env: BTreeMap::new(),
            declarative_globals: None,
        })
    }

    pub(crate) async fn init() -> Result<Self> {
        async { Self::new() }.await
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Result<Self> {
        Self::new()
    }

    fn scenario_dir(&self) -> std::io::Result<Dir> {
        Dir::open_ambient_dir(self.workdir.path(), cap_std::ambient_authority())
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
