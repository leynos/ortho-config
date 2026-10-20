//! Isolated Cargo commands for executable documentation tests.
//!
//! Shared test infrastructure applies a closed build-tool environment and
//! reuses Cargo's resolved target with separate documentation ownership state.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
use crate::process_runner::{self, Operation, Policy};
#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
use cargo_environment::CARGO_ENV_ALLOWLIST;
use cargo_environment::CargoEnvironment;

#[path = "cargo_environment.rs"]
pub(crate) mod cargo_environment;

#[derive(Default)]
struct PreparedCargoEnvironment(Vec<(OsString, OsString)>);

/// Prepend a private subcommand directory to the optional inherited search path.
pub(crate) fn cargo_dispatch_path(
    fixture_bin_dir: &Path,
    inherited_path: Option<&OsStr>,
) -> Result<OsString> {
    std::env::join_paths(
        std::iter::once(fixture_bin_dir.to_path_buf())
            .chain(inherited_path.into_iter().flat_map(std::env::split_paths)),
    )
    .context("construct PATH for Cargo dispatch")
}

/// Prepare build-tool state while reusing an independently resolved target.
pub(crate) fn prepare_cargo_command_for_target_before(
    working_directory: &Path,
    state_directory: &Path,
    target_directory: &Path,
    deadline: Instant,
) -> Result<Command> {
    let prepared_environment = prepare_cargo_environment(state_directory, deadline)?;
    remaining_discovery_time(deadline, "prepare Cargo command")?;
    let mut command = cargo_command(
        working_directory,
        Some(target_directory),
        &CargoEnvironment::build(),
    );
    command.envs(prepared_environment.0);
    Ok(command)
}

/// Prepare Cargo metadata without changing its caller-selected target paths.
///
/// The caller's target settings affect discovery only; later builds use an
/// explicitly owned target directory.
///
pub(crate) fn prepare_metadata_command(working_directory: &Path) -> Command {
    let environment = CargoEnvironment::metadata();
    cargo_command(working_directory, None, &environment)
}

pub(crate) fn cargo_command(
    working_directory: &Path,
    target_directory_override: Option<&Path>,
    environment: &CargoEnvironment,
) -> Command {
    let mut command = Command::new("cargo");
    environment.apply(&mut command);
    command.current_dir(working_directory);
    if let Some(target_directory) = target_directory_override {
        command.env("CARGO_TARGET_DIR", target_directory);
    }
    command
}

fn prepare_cargo_environment(
    state_directory: &Path,
    deadline: Instant,
) -> Result<PreparedCargoEnvironment> {
    Dir::open_ambient_dir(state_directory, ambient_authority())
        .context("open isolated Cargo state directory")?;
    #[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
    {
        prepare_msvc_environment(state_directory, deadline)
    }
    #[cfg(not(all(windows, target_env = "msvc", target_arch = "x86_64")))]
    {
        remaining_discovery_time(deadline, "prepare Cargo environment")?;
        Ok(PreparedCargoEnvironment::default())
    }
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn prepare_msvc_environment(
    state_directory: &Path,
    deadline: Instant,
) -> Result<PreparedCargoEnvironment> {
    let Some((linker, vcvars)) = find_msvc_toolchain(deadline)? else {
        return Ok(PreparedCargoEnvironment::default());
    };
    let mut variables = vec![(
        OsString::from("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"),
        linker,
    )];
    variables.extend(
        vcvars_environment(state_directory, &vcvars, deadline)?
            .into_iter()
            .map(|(name, value)| (name.into(), value.into())),
    );
    Ok(PreparedCargoEnvironment(variables))
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn find_msvc_toolchain(deadline: Instant) -> Result<Option<(OsString, std::path::PathBuf)>> {
    let Some(vswhere) = vswhere_path() else {
        return Ok(None);
    };
    let installation_output = run_vswhere(&vswhere, &["-property", "installationPath"], deadline)?;
    let linker_output = run_vswhere(
        &vswhere,
        &["-find", r"VC\Tools\MSVC\**\bin\Hostx64\x64\link.exe"],
        deadline,
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
fn run_vswhere(vswhere: &Path, query: &[&str], deadline: Instant) -> Result<Vec<u8>> {
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
    Ok(run_process_before(
        &mut command,
        Operation("discover the MSVC toolchain"),
        deadline,
    )?
    .stdout)
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn vcvars_environment(
    state_directory: &Path,
    vcvars: &Path,
    deadline: Instant,
) -> Result<Vec<(String, String)>> {
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
    let output = run_process_before(
        &mut command,
        Operation("prepare the MSVC environment"),
        deadline,
    )?;
    let environment = decode_utf16le(&output.stdout)?;
    Ok(allowed_environment(&environment))
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn decode_utf16le(output: &[u8]) -> Result<String> {
    let (pairs, remainder) = output.as_chunks::<2>();
    ensure!(
        remainder.is_empty(),
        "MSVC environment output should contain complete UTF-16 code units"
    );
    let code_units = pairs
        .iter()
        .map(|pair| {
            let [low, high] = pair;
            u16::from(*low) | (u16::from(*high) << 8)
        })
        .collect::<Vec<_>>();
    String::from_utf16(&code_units).context("MSVC environment output should be valid UTF-16")
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
fn run_process_before(
    command: &mut Command,
    operation: Operation<'_>,
    deadline: Instant,
) -> Result<std::process::Output> {
    CargoEnvironment::build().apply(command);
    let remaining = remaining_discovery_time(deadline, operation.0)?;
    let policy = Policy::discovery().bounded_by(remaining);
    let output = process_runner::run_with_policy(command, operation, policy)?;
    let Operation(operation_name) = operation;
    ensure!(
        output.status.success(),
        "{operation_name}: subprocess failed with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(output)
}

fn remaining_discovery_time(deadline: Instant, operation: &str) -> Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(
        !remaining.is_zero(),
        "{operation}: Cargo discovery deadline elapsed"
    );
    Ok(remaining)
}

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
pub(super) fn allowed_environment(environment: &str) -> Vec<(String, String)> {
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

#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
pub(super) fn first_output_line(output: &[u8]) -> Option<&OsStr> {
    let decoded_output = std::str::from_utf8(output).ok()?;
    decoded_output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(OsStr::new)
}
