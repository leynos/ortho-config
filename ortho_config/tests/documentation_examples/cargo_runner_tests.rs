//! Regression coverage for Cargo command preparation and host discovery.

use super::{
    CARGO_ENV_ALLOWLIST, allowed_environment, cargo_command, first_output_line,
    prepare_cargo_command, run_process, run_process_before,
};
use crate::process_runner::{self, Operation, Policy};
use anyhow::ensure;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

const SECRET_NAME: &str = "ORTHO_CONFIG_DOCUMENTATION_TEST_SECRET";

#[test]
fn build_command_keeps_shared_cache_and_uses_its_owned_target_directory() -> anyhow::Result<()> {
    let state = TempDir::new()?;
    let supplied = HashMap::from([
        ("CARGO_HOME", "/home/agent/.cargo"),
        ("CARGO_TARGET_DIR", "/host/target"),
        ("CARGO_BUILD_TARGET_DIR", "/configured/target"),
        ("RUSTC_WRAPPER", "/usr/bin/build-limits-rustc"),
        ("CARGO_BUILD_JOBS", "4"),
        ("RUSTFLAGS", "-D warnings"),
        (SECRET_NAME, "must-not-leak"),
    ]);
    let environment =
        super::cargo_environment::CargoEnvironment::from_lookup(CARGO_ENV_ALLOWLIST, |name| {
            supplied.get(name).map(OsString::from)
        });
    let target_directory = state.path().join("target");
    let command = cargo_command(
        Path::new("/repository"),
        Some(&target_directory),
        &environment,
    );

    ensure!(
        command_env(&command, "CARGO_HOME") == Some(OsStr::new("/home/agent/.cargo")),
        "Cargo should retain the default shared package cache"
    );
    ensure!(
        command_env(&command, "CARGO_TARGET_DIR") == Some(target_directory.as_os_str()),
        "the build should use its explicitly owned target directory"
    );
    ensure!(command_env(&command, "CARGO_BUILD_TARGET_DIR").is_none());
    ensure!(command_env(&command, "RUSTC_WRAPPER").is_some());
    ensure!(command_env(&command, "CARGO_BUILD_JOBS").is_some());
    ensure!(command_env(&command, "RUSTFLAGS").is_some());
    ensure!(command_env(&command, SECRET_NAME).is_none());
    Ok(())
}

#[test]
fn metadata_command_preserves_caller_target_directories() {
    let supplied = HashMap::from([
        ("CARGO_HOME", "/home/agent/.cargo"),
        ("CARGO_TARGET_DIR", "/workspace/target"),
        ("CARGO_BUILD_TARGET_DIR", "/configured/target"),
    ]);
    let environment = super::cargo_environment::CargoEnvironment::metadata_with_lookup(|name| {
        supplied.get(name).map(OsString::from)
    });
    let command = cargo_command(Path::new("/repository"), None, &environment);

    assert_eq!(
        command_env(&command, "CARGO_TARGET_DIR"),
        Some(OsStr::new("/workspace/target"))
    );
    assert_eq!(
        command_env(&command, "CARGO_BUILD_TARGET_DIR"),
        Some(OsStr::new("/configured/target"))
    );
}

#[test]
fn cargo_child_does_not_observe_an_application_secret() {
    let mut command = current_test_executable();
    command.env(SECRET_NAME, "must-not-leak");
    super::cargo_environment::CargoEnvironment::build().apply(&mut command);
    command.args(["--ignored", "--nocapture", "cargo_environment_probe"]);
    let output = process_runner::run_with_policy(
        &mut command,
        Operation("run sanitized build environment probe"),
        Policy::discovery(),
    )
    .expect("the sanitized build environment probe should run");
    assert!(output.status.success(), "probe failed: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("{SECRET_NAME}=<absent>")),
        "application secret reached the build child: {output:?}"
    );
}

#[test]
#[ignore = "executed in a sanitized child process"]
fn cargo_environment_probe() {
    use std::io::Write;

    let value = std::env::var(SECRET_NAME).unwrap_or_else(|_| "<absent>".to_owned());
    writeln!(std::io::stdout().lock(), "{SECRET_NAME}={value}")
        .expect("write environment-probe output");
}

#[test]
fn discovery_process_start_failures_are_reported() {
    let mut command = Command::new("ortho-config-documentation-command-that-does-not-exist");
    let error = run_process(&mut command, Operation("discover the MSVC toolchain"))
        .expect_err("a missing discovery command should return an error");
    let message = format!("{error:#}");
    assert!(message.contains("discover the MSVC toolchain: start subprocess"));
}

#[test]
fn preparation_process_failures_are_reported() {
    let mut command = current_test_executable();
    command.args(["--ignored", "--nocapture", "process_failure_probe"]);
    let error = run_process(&mut command, Operation("prepare the MSVC environment"))
        .expect_err("a failed preparation command should return an error");
    let message = format!("{error:#}");
    assert!(message.contains("prepare the MSVC environment: subprocess failed"));
}

#[test]
fn discovery_rejects_an_expired_overall_deadline_before_spawning() {
    let mut command = Command::new("ortho-config-documentation-command-that-does-not-exist");
    let error = run_process_before(
        &mut command,
        Operation("discover the MSVC toolchain"),
        Instant::now(),
    )
    .expect_err("an expired discovery deadline should not spawn a child");
    assert!(format!("{error:#}").contains("Cargo discovery deadline elapsed"));
}

#[test]
#[ignore = "executed as a failing subprocess"]
fn process_failure_probe() {
    std::process::exit(23);
}

#[test]
fn concurrent_cargo_preparation_isolates_state_directories() -> anyhow::Result<()> {
    let first_state = TempDir::new()?;
    let second_state = TempDir::new()?;
    let (first_result, second_result) = std::thread::scope(|scope| {
        let first = scope.spawn(|| prepare_cargo_command(first_state.path(), first_state.path()));
        let second =
            scope.spawn(|| prepare_cargo_command(second_state.path(), second_state.path()));
        (
            first
                .join()
                .expect("first Cargo preparation should not panic"),
            second
                .join()
                .expect("second Cargo preparation should not panic"),
        )
    });
    let first_command = first_result?;
    let second_command = second_result?;
    let first_target = first_state.path().join("target");
    let second_target = second_state.path().join("target");
    ensure!(
        command_env(&first_command, "CARGO_TARGET_DIR") == Some(first_target.as_os_str()),
        "first Cargo command should use its own state directory"
    );
    ensure!(
        command_env(&second_command, "CARGO_TARGET_DIR") == Some(second_target.as_os_str()),
        "second Cargo command should use its own state directory"
    );
    ensure!(
        command_env(&first_command, "CARGO_TARGET_DIR")
            != command_env(&second_command, "CARGO_TARGET_DIR"),
        "concurrent Cargo commands should not share state directories"
    );
    Ok(())
}

#[test]
fn visual_studio_discovery_uses_the_first_reported_linker() {
    let output = b"C:\\Visual Studio\\link.exe\r\nC:\\Other\\link.exe\r\n";
    assert_eq!(
        first_output_line(output),
        Some(OsStr::new(r"C:\Visual Studio\link.exe"))
    );
}

#[test]
fn visual_studio_environment_keeps_only_build_variables() {
    let environment = concat!(
        "LIB=C:\\Windows Kits\\Lib\r\n",
        "Path=C:\\Visual Studio\\bin\r\n",
        "SECRET=do-not-copy\r\n",
    );
    assert_eq!(
        allowed_environment(environment),
        vec![
            ("LIB".to_owned(), r"C:\Windows Kits\Lib".to_owned()),
            ("PATH".to_owned(), r"C:\Visual Studio\bin".to_owned()),
        ]
    );
}

fn current_test_executable() -> Command {
    Command::new(
        std::env::current_exe().expect("the integration-test executable should have a path"),
    )
}

fn command_env<'a>(command: &'a Command, key: &str) -> Option<&'a OsStr> {
    command
        .get_envs()
        .find(|(name, _)| *name == OsStr::new(key))
        .and_then(|(_, value)| value)
}
