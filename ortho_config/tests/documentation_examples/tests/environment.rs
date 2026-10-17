//! Environment-boundary probes isolated from parallel documentation tests.
//!
//! Only child processes receive budget overrides. Each child exercises the real
//! runner, while an independent outer deadline bounds a broken wiring path.

use super::super::{Operation, ProcessLimits, TIMEOUT_ENV, run_command, run_command_with_limits};
use super::probe_command;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::io::Write;
use std::process::Command;
use std::time::Duration;

/// Select the contract checked inside an isolated integration-test process.
#[derive(Clone, Copy)]
enum Probe {
    /// Accepted budgets must preserve the independent per-stream capture bound.
    Capture,
    /// Configuration errors must win over an attempted executable lookup.
    Invalid,
    /// A valid override must change the deadline used by the real runner.
    Deadline,
}

impl Probe {
    /// Use unique fixture names so the test harness runs exactly one probe.
    fn test_name(self) -> &'static str {
        match self {
            Self::Capture => "environment_capture_contract_probe",
            Self::Invalid => "environment_invalid_contract_probe",
            Self::Deadline => "environment_deadline_contract_probe",
        }
    }
}

/// Exercise actual environment reads without changing the parent test process.
#[rstest]
#[case::absent(None, Probe::Capture)]
#[case::minimum(Some("1"), Probe::Capture)]
#[case::shared_host(Some("7200"), Probe::Capture)]
#[case::maximum(Some("86400"), Probe::Capture)]
#[case::empty(Some(""), Probe::Invalid)]
#[case::zero(Some("0"), Probe::Invalid)]
#[case::negative(Some("-1"), Probe::Invalid)]
#[case::not_seconds(Some("forever"), Probe::Invalid)]
#[case::over_limit(Some("86401"), Probe::Invalid)]
#[case::overflow(Some("18446744073709551616"), Probe::Invalid)]
#[case::deadline(Some("1"), Probe::Deadline)]
fn timeout_environment_is_applied(#[case] value: Option<&str>, #[case] probe: Probe) -> Result<()> {
    let mut command = probe_command(probe.test_name())?;
    command.env_remove(TIMEOUT_ENV);
    if let Some(timeout_text) = value {
        command.env(TIMEOUT_ENV, timeout_text);
    }
    assert_probe(&mut command)
}

/// Non-Unicode values must fail validation before any spawn attempt on Unix.
#[cfg(unix)]
#[test]
fn non_unicode_timeout_environment_is_rejected() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let mut command = probe_command(Probe::Invalid.test_name())?;
    command.env(TIMEOUT_ENV, OsString::from_vec(vec![0xff]));
    assert_probe(&mut command)
}

/// Bound fixture failures independently of the environment-driven inner runner.
fn assert_probe(command: &mut Command) -> Result<()> {
    let output = run_command_with_limits(
        command,
        Operation("check environment-boundary probe"),
        ProcessLimits {
            timeout: Duration::from_secs(10),
            output_bytes: 16 * 1024,
        },
    )?;
    ensure!(
        output.status.success(),
        "environment probe failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}

/// Accepted overrides and absence retain a one-MiB bound on each stream.
#[test]
#[ignore = "executed in a subprocess with an isolated environment"]
fn environment_capture_contract_probe() -> Result<()> {
    let mut command = probe_command("environment_large_output_probe")?;
    let output = run_command(&mut command, Operation("capture environment probe output"))?;
    ensure!(output.status.success(), "output probe failed: {output:?}");
    // Keep this oracle independent of the implementation's default constant.
    ensure!(
        output.stdout.len() == 1024 * 1024,
        "stdout capture bound changed"
    );
    ensure!(
        output.stderr.len() == 1024 * 1024,
        "stderr capture bound changed"
    );
    Ok(())
}

/// A nonexistent executable distinguishes validation from a spawn failure.
#[test]
#[ignore = "executed in a subprocess with an isolated environment"]
fn environment_invalid_contract_probe() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut command = Command::new(directory.path().join("nonexistent-probe-executable"));
    let error = run_command(&mut command, Operation("reject invalid environment"))
        .err()
        .context("invalid environment must fail before spawning")?;
    let diagnostic = format!("{error:#}");
    ensure!(diagnostic.contains(TIMEOUT_ENV), "{diagnostic}");
    ensure!(!diagnostic.contains("start subprocess"), "{diagnostic}");
    Ok(())
}

/// A one-second override must terminate a child that would otherwise finish.
#[test]
#[ignore = "executed in a subprocess with an isolated environment"]
fn environment_deadline_contract_probe() -> Result<()> {
    let mut command = probe_command("environment_short_sleep_probe")?;
    let error = run_command(&mut command, Operation("apply environment deadline"))
        .err()
        .context("one-second budget must interrupt the three-second probe")?;
    ensure!(
        format!("{error:#}").contains("subprocess timed out after 1s"),
        "unexpected deadline failure: {error:#}",
    );
    Ok(())
}

/// Exceed the capture bound on both pipes without allocating a large buffer.
#[test]
#[ignore = "executed as an output fixture by the environment capture probe"]
fn environment_large_output_probe() -> Result<()> {
    let chunk = [b'x'; 8192];
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    for _ in 0..129 {
        stdout.write_all(&chunk)?;
        stderr.write_all(&chunk)?;
    }
    Ok(())
}

/// Finish promptly even if a regression removes environment deadline wiring.
#[test]
#[ignore = "executed as a finite sleep fixture by the environment deadline probe"]
fn environment_short_sleep_probe() {
    std::thread::sleep(Duration::from_secs(3));
}
