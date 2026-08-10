//! Isolated Cargo commands for executable documentation tests.
//!
//! This module is test infrastructure shared only by targets that execute
//! documented Cargo workflows. Callers supply a temporary state directory;
//! the runner clears the inherited environment, restores the closed toolchain
//! allow-list, and keeps build artefacts out of the repository target tree.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

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

/// Create a Cargo command with isolated process state and build artefacts.
pub(super) fn cargo_command(working_directory: &Path, state_directory: &Path) -> Command {
    let mut command = Command::new("cargo");
    sanitize_environment(&mut command, CARGO_ENV_ALLOWLIST);
    command
        .current_dir(working_directory)
        .env("CARGO_TARGET_DIR", state_directory.join("target"));
    #[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
    configure_msvc_linker(&mut command, state_directory);
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

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn configure_msvc_linker(command: &mut Command, state_directory: &Path) {
    if let Some((linker, vcvars)) = find_msvc_toolchain() {
        command.env("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER", linker);
        if let Some(environment) = vcvars_environment(state_directory, &vcvars) {
            command.envs(environment);
        }
    }
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn find_msvc_toolchain() -> Option<(std::ffi::OsString, std::path::PathBuf)> {
    let vswhere = vswhere_path()?;
    let installation_output = run_vswhere(&vswhere, &["-property", "installationPath"])?;
    let linker_output = run_vswhere(
        &vswhere,
        &["-find", r"VC\Tools\MSVC\**\bin\Hostx64\x64\link.exe"],
    )?;
    let installation_path = first_output_line(&installation_output)?.to_str()?;
    let linker_path = first_output_line(&linker_output)?.to_os_string();
    let vcvars = Path::new(installation_path)
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat");
    Some((linker_path, vcvars))
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
fn run_vswhere(vswhere: &Path, query: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new(vswhere)
        .args(
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
        )
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn vcvars_environment(state_directory: &Path, vcvars: &Path) -> Option<Vec<(String, String)>> {
    use cap_std::{ambient_authority, fs::Dir};

    let directory = Dir::open_ambient_dir(state_directory, ambient_authority()).ok()?;
    let script_name = "msvc-environment.cmd";
    let script = format!("@call \"{}\" >nul\r\n@set\r\n", vcvars.display());
    directory.write(script_name, script).ok()?;
    let output = Command::new("cmd.exe")
        .args(["/d", "/u", "/c"])
        .arg(state_directory.join(script_name))
        .output()
        .ok()?;
    let environment = output
        .status
        .success()
        .then(|| decode_utf16le(&output.stdout))??;
    Some(allowed_environment(&environment))
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn decode_utf16le(output: &[u8]) -> Option<String> {
    let mut chunks = output.chunks_exact(2);
    let code_units = chunks
        .by_ref()
        .filter_map(|pair| match pair {
            [low, high] => Some(u16::from(*low) | (u16::from(*high) << 8)),
            _ => None,
        })
        .collect::<Vec<_>>();
    chunks
        .remainder()
        .is_empty()
        .then(|| String::from_utf16(&code_units).ok())
        .flatten()
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
        CARGO_ENV_ALLOWLIST, allowed_environment, first_output_line, sanitize_environment,
    };
    use std::process::Command;

    const SECRET_NAME: &str = "ORTHO_CONFIG_DOCUMENTATION_TEST_SECRET";

    #[test]
    fn cargo_child_does_not_observe_excluded_environment_values() {
        let mut command = Command::new(
            std::env::current_exe().expect("the integration-test executable should have a path"),
        );
        command.env(SECRET_NAME, "must-not-leak");
        sanitize_environment(&mut command, CARGO_ENV_ALLOWLIST);
        let output = command
            .args(["--ignored", "--nocapture", "cargo_environment_probe"])
            .output()
            .expect("the environment probe should run");
        assert!(
            output.status.success(),
            "environment probe failed: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("{SECRET_NAME}=<absent>")),
            "excluded environment value reached the Cargo child: {output:?}"
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
