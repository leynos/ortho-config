//! Run a process-backed code path in a child process instead of mutating ours.
//!
//! Some public methods read the live process environment and working directory
//! by design (`load_and_merge`, `load_from_iter`, and the `_with_sources`
//! forms that keep process-backed file discovery). A test of one of them must
//! control both, and changing them in the test process would race every other
//! test in it. The harness re-runs the test binary as a child with an exact
//! environment and working directory, so the parent mutates nothing.
//!
//! # Protocol
//!
//! A *child test* is an ordinary `#[test]` that returns `Ok(())` straight away
//! unless [`case`] reports a case name; the parent test calls [`run`] with the
//! child test's name and the case to exercise. The child therefore passes
//! trivially in a normal run and does its real work only when started here.

use anyhow::{Context, Result, ensure};
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Name of the variable that carries the case to the child.
const CASE_VARIABLE: &str = "ORTHO_PROCESS_CHILD_CASE";
/// Upper bound for one child run, including cleanup.
const CHILD_DEADLINE: Duration = Duration::from_secs(120);

/// The case this process was started to run, or `None` in a normal run.
pub fn case() -> Option<String> {
    std::env::var(CASE_VARIABLE).ok()
}

/// What the child is run with: a working directory and an exact environment.
pub struct ChildEnvironment<'a> {
    /// Working directory of the child.
    pub dir: &'a Path,
    /// Directory every home and platform-configuration variable points at.
    ///
    /// All of `HOME`, `USERPROFILE`, `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS` and
    /// `APPDATA` are set, so no host directory is reachable on any platform.
    pub empty_root: &'a Path,
    /// Further variables the case needs.
    pub vars: &'a [(&'a str, OsString)],
}

/// Run `test` in a child as `case` and require it to pass.
///
/// `test` is the child test's exact name in this binary.
///
/// # Errors
///
/// Returns an error if the child cannot be started, exceeds its deadline, or
/// exits unsuccessfully; the error carries the child's output.
pub fn run(test: &str, case: &str, environment: &ChildEnvironment<'_>) -> Result<()> {
    let executable = std::env::current_exe().context("locate the test binary")?;
    let mut command = Command::new(executable);
    command
        .args(["--exact", test, "--nocapture"])
        .env_clear()
        .current_dir(environment.dir)
        .env(CASE_VARIABLE, case)
        .env("HOME", environment.empty_root)
        .env("USERPROFILE", environment.empty_root)
        .env("XDG_CONFIG_HOME", environment.empty_root.join(".config"))
        .env("XDG_CONFIG_DIRS", environment.empty_root)
        .env("APPDATA", environment.empty_root);
    // Windows cannot start a process without its system root.
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    for (name, value) in environment.vars {
        command.env(name, value);
    }

    let output =
        test_helpers::process::run_with_deadline(command, CHILD_DEADLINE, "process-backed child")?;
    ensure!(
        output.status.success(),
        "child case `{case}` failed: {output:?}"
    );
    ensure!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "child case `{case}` did not run its test: {output:?}"
    );
    Ok(())
}
