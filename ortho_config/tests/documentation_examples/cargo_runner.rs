//! Isolated Cargo commands for executable documentation tests.
//!
//! Shared test infrastructure clears inherited state, restores a closed
//! toolchain allow-list, and keeps artefacts out of the repository target tree.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Output};

const CARGO_ENV_ALLOWLIST: &[&str] = &[
    "CARGO_HOME",
    "HOME",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "PATH",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SYSTEMROOT",
    "TMPDIR",
    "USERPROFILE",
    "VCINSTALLDIR",
    "VSCMD_ARG_TGT_ARCH",
    "VSINSTALLDIR",
    "WINDIR",
    "WindowsSDKVersion",
    "WindowsSdkDir",
];

#[derive(Default)]
struct PreparedCargoEnvironment(Vec<(OsString, OsString)>);

/// Prepare and create a Cargo command with isolated process state.
///
/// # Errors
///
/// Returns an error when state validation or Windows preparation fails.
pub(super) fn prepare_cargo_command(
    working_directory: &Path,
    state_directory: &Path,
) -> Result<Command> {
    let environment = prepare_cargo_environment(state_directory)?;
    Ok(cargo_command(
        working_directory,
        state_directory,
        &environment,
    ))
}

fn cargo_command(
    working_directory: &Path,
    state_directory: &Path,
    environment: &PreparedCargoEnvironment,
) -> Command {
    let mut command = Command::new("cargo");
    sanitize_environment(&mut command, CARGO_ENV_ALLOWLIST);
    command
        .current_dir(working_directory)
        .env("CARGO_TARGET_DIR", state_directory.join("target"))
        .envs(environment.0.iter().map(|(name, value)| (name, value)));
    command
}

/// Replace a command's environment with values from a closed host allow-list.
pub(super) fn sanitize_environment(command: &mut Command, allowlist: &[&str]) {
    command.env_clear();
    for name in allowlist {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn prepare_cargo_environment(state_directory: &Path) -> Result<PreparedCargoEnvironment> {
    Dir::open_ambient_dir(state_directory, ambient_authority())
        .context("open isolated Cargo state directory")?;
    #[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
    {
        prepare_msvc_environment(state_directory)
    }
    #[cfg(not(all(windows, target_env = "msvc", target_arch = "x86_64")))]
    Ok(PreparedCargoEnvironment::default())
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn prepare_msvc_environment(state_directory: &Path) -> Result<PreparedCargoEnvironment> {
    let Some((linker, vcvars)) = find_msvc_toolchain()? else {
        return Ok(PreparedCargoEnvironment::default());
    };
    let mut variables = vec![(
        OsString::from("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"),
        linker,
    )];
    variables.extend(
        vcvars_environment(state_directory, &vcvars)?
            .into_iter()
            .map(|(name, value)| (name.into(), value.into())),
    );
    Ok(PreparedCargoEnvironment(variables))
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn find_msvc_toolchain() -> Result<Option<(OsString, std::path::PathBuf)>> {
    let Some(vswhere) = vswhere_path() else {
        return Ok(None);
    };
    let installation_output = run_vswhere(&vswhere, &["-property", "installationPath"])?;
    let linker_output = run_vswhere(
        &vswhere,
        &["-find", r"VC\Tools\MSVC\**\bin\Hostx64\x64\link.exe"],
    )?;
    let installation_path = first_output_line(&installation_output)
        .context("vswhere returned no Visual Studio installation path")?
        .to_str()
        .context("Visual Studio installation path should be UTF-8")?;
    let linker_path = first_output_line(&linker_output)
        .context("vswhere returned no MSVC linker path")?
        .to_os_string();
    let vcvars = Path::new(installation_path)
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat");
    Ok(Some((linker_path, vcvars)))
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn vswhere_path() -> Option<std::path::PathBuf> {
    let program_files =
        std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))?;
    Some(
        Path::new(&program_files)
            .join("Microsoft Visual Studio")
            .join("Installer")
            .join("vswhere.exe"),
    )
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn run_vswhere(vswhere: &Path, query: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new(vswhere);
    command.args(
        [
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        ]
        .into_iter()
        .chain(query.iter().copied())
        .chain(["-utf8"]),
    );
    Ok(run_process(&mut command, "discover the MSVC toolchain")?.stdout)
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn vcvars_environment(state_directory: &Path, vcvars: &Path) -> Result<Vec<(String, String)>> {
    let directory = Dir::open_ambient_dir(state_directory, ambient_authority())
        .context("open isolated Cargo state directory")?;
    let script_name = "msvc-environment.cmd";
    let script = format!("@call \"{}\" >nul\r\n@set\r\n", vcvars.display());
    directory
        .write(script_name, script)
        .context("write MSVC environment command file")?;
    let mut command = Command::new("cmd.exe");
    command
        .args(["/d", "/u", "/c"])
        .arg(state_directory.join(script_name));
    let output = run_process(&mut command, "prepare the MSVC environment")?;
    let environment = decode_utf16le(&output.stdout)?;
    Ok(allowed_environment(&environment))
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn decode_utf16le(output: &[u8]) -> Result<String> {
    let mut chunks = output.chunks_exact(2);
    let code_units = chunks
        .by_ref()
        .filter_map(|pair| match pair {
            [low, high] => Some(u16::from(*low) | (u16::from(*high) << 8)),
            _ => None,
        })
        .collect::<Vec<_>>();
    ensure!(
        chunks.remainder().is_empty(),
        "MSVC environment output should contain complete UTF-16 code units"
    );
    String::from_utf16(&code_units).context("MSVC environment output should be valid UTF-16")
}

fn run_process(command: &mut Command, operation: &str) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("{operation}: start subprocess"))?;
    ensure!(
        output.status.success(),
        "{operation}: subprocess failed with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(output)
}

fn allowed_environment(environment: &str) -> Vec<(String, String)> {
    environment
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter_map(|(name, value)| {
            CARGO_ENV_ALLOWLIST
                .iter()
                .find(|allowed| allowed.eq_ignore_ascii_case(name))
                .map(|allowed| ((*allowed).to_owned(), value.to_owned()))
        })
        .collect()
}

fn first_output_line(output: &[u8]) -> Option<&OsStr> {
    let decoded_output = std::str::from_utf8(output).ok()?;
    decoded_output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(OsStr::new)
}

#[cfg(test)]
mod tests {
    //! Regression coverage for the isolated Cargo process boundary.

    use super::{
        CARGO_ENV_ALLOWLIST, allowed_environment, first_output_line, prepare_cargo_command,
        run_process, sanitize_environment,
    };
    use anyhow::ensure;
    use std::ffi::OsStr;
    use std::process::Command;
    use tempfile::TempDir;

    const SECRET_NAME: &str = "ORTHO_CONFIG_DOCUMENTATION_TEST_SECRET";

    #[test]
    fn cargo_child_does_not_observe_inherited_environment_values() {
        let mut command = Command::new(
            std::env::current_exe().expect("the integration-test executable should have a path"),
        );
        command.env(SECRET_NAME, "must-not-leak");
        let output = command
            .args([
                "--ignored",
                "--nocapture",
                "cargo_environment_sanitizer_probe",
            ])
            .output()
            .expect("the inherited-environment probe should run");
        assert!(
            output.status.success(),
            "inherited-environment probe failed: {output:?}"
        );
    }

    #[test]
    #[ignore = "executed in a sanitized child process"]
    fn cargo_environment_sanitizer_probe() {
        assert_eq!(
            std::env::var(SECRET_NAME).as_deref(),
            Ok("must-not-leak"),
            "the probe process should inherit the excluded value"
        );
        let mut command = Command::new(
            std::env::current_exe().expect("the integration-test executable should have a path"),
        );
        sanitize_environment(&mut command, CARGO_ENV_ALLOWLIST);
        let output = command
            .args(["--ignored", "--nocapture", "cargo_environment_probe"])
            .output()
            .expect("the sanitized environment probe should run");
        assert!(
            output.status.success(),
            "sanitized environment probe failed: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("{SECRET_NAME}=<absent>")),
            "inherited environment value reached the sanitized child: {output:?}"
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
        let error = run_process(&mut command, "discover the MSVC toolchain")
            .expect_err("a missing discovery command should return an error");
        let message = format!("{error:#}");
        assert!(message.contains("discover the MSVC toolchain: start subprocess"));
    }

    #[test]
    fn preparation_process_failures_are_reported() {
        let mut command = Command::new(
            std::env::current_exe().expect("the integration-test executable should have a path"),
        );
        command.args(["--ignored", "--nocapture", "process_failure_probe"]);
        let error = run_process(&mut command, "prepare the MSVC environment")
            .expect_err("a failed preparation command should return an error");
        let message = format!("{error:#}");
        assert!(message.contains("prepare the MSVC environment: subprocess failed"));
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
            let first =
                scope.spawn(|| prepare_cargo_command(first_state.path(), first_state.path()));
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
            target_directory(&first_command) == Some(first_target.as_os_str()),
            "first Cargo command should use its own state directory"
        );
        ensure!(
            target_directory(&second_command) == Some(second_target.as_os_str()),
            "second Cargo command should use its own state directory"
        );
        ensure!(
            target_directory(&first_command) != target_directory(&second_command),
            "concurrent Cargo commands should not share state directories"
        );
        Ok(())
    }

    fn target_directory(command: &Command) -> Option<&OsStr> {
        command
            .get_envs()
            .find(|(name, _)| *name == OsStr::new("CARGO_TARGET_DIR"))
            .and_then(|(_, value)| value)
    }

    #[test]
    fn visual_studio_discovery_uses_the_first_reported_linker() {
        let output = b"C:\\Visual Studio\\link.exe\r\nC:\\Other\\link.exe\r\n";
        assert_eq!(
            first_output_line(output),
            Some(std::ffi::OsStr::new(r"C:\Visual Studio\link.exe"))
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
}
