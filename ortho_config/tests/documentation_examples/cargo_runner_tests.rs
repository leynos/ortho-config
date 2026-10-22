//! Regression coverage for Cargo command preparation and host discovery.

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
use super::msvc_environment_is_prepared;
use super::{CARGO_ENV_ALLOWLIST, cargo_command, prepare_cargo_command_for_target_before};
#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
use super::{allowed_environment, first_output_line};
use crate::process_runner::{self, Operation, Policy};
use anyhow::ensure;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
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
fn cargo_child_does_not_observe_an_application_secret() -> anyhow::Result<()> {
    let mut command = current_test_executable()?;
    command.env(SECRET_NAME, "must-not-leak");
    super::cargo_environment::CargoEnvironment::build().apply(&mut command);
    command.args(["--ignored", "--nocapture", "cargo_environment_probe"]);
    let output = process_runner::run_with_policy(
        &mut command,
        Operation("run sanitized build environment probe"),
        Policy::discovery(),
    )?;
    ensure!(output.status.success(), "probe failed: {output:?}");
    ensure!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("{SECRET_NAME}=<absent>")),
        "application secret reached the build child: {output:?}"
    );
    Ok(())
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
fn concurrent_cargo_preparation_isolates_state_directories() -> anyhow::Result<()> {
    let first_state = TempDir::new()?;
    let second_state = TempDir::new()?;
    let first_target = first_state.path().join("target");
    let second_target = second_state.path().join("target");
    let deadline = Instant::now() + Duration::from_secs(30);
    let (first_result, second_result) = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            prepare_cargo_command_for_target_before(
                first_state.path(),
                first_state.path(),
                &first_target,
                deadline,
            )
        });
        let second = scope.spawn(|| {
            prepare_cargo_command_for_target_before(
                second_state.path(),
                second_state.path(),
                &second_target,
                deadline,
            )
        });
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

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
#[test]
fn visual_studio_discovery_uses_the_first_reported_linker() {
    let output = b"C:\\Visual Studio\\link.exe\r\nC:\\Other\\link.exe\r\n";
    assert_eq!(
        first_output_line(output),
        Some(OsStr::new(r"C:\Visual Studio\link.exe"))
    );
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
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

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
#[test]
fn msvc_environment_requires_linker_and_sdk_paths() {
    let complete = HashMap::from([
        (
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
            OsString::from(r"C:\Visual Studio\link.exe"),
        ),
        ("INCLUDE", OsString::from(r"C:\Visual Studio\include")),
        ("LIB", OsString::from(r"C:\Windows Kits\Lib")),
    ]);

    assert!(msvc_environment_is_prepared(|name| complete
        .get(name)
        .cloned()));
    for missing in [
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
        "INCLUDE",
        "LIB",
    ] {
        let mut incomplete = complete.clone();
        incomplete.remove(missing);
        assert!(
            !msvc_environment_is_prepared(|name| incomplete.get(name).cloned()),
            "missing {missing} should require Visual Studio discovery"
        );
    }
}

fn current_test_executable() -> anyhow::Result<Command> {
    Ok(Command::new(std::env::current_exe()?))
}

fn command_env<'a>(command: &'a Command, key: &str) -> Option<&'a OsStr> {
    command
        .get_envs()
        .find(|(name, _)| *name == OsStr::new(key))
        .and_then(|(_, value)| value)
}
