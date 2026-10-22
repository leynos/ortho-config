//! Separate-process build-and-snapshot regression without a live compiler.
//!
//! Controlled script executables stand in for compiler output. Their embedded
//! identities differ even though their shared final path is identical.

use super::{BuildCache, BuildRequest};
use crate::process_runner::{self, Operation, Policy};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(unix)]
const SCRIPT_NAME: &str = "same-name.sh";
#[cfg(windows)]
const SCRIPT_NAME: &str = "same-name.cmd";

#[cfg(unix)]
const COMPILER_SOURCE: &str = "cp \"$1\" \"$2\" && cat \"$3\"\n";
#[cfg(windows)]
const COMPILER_SOURCE: &str = concat!(
    "@copy /y \"%~1\" \"%~2\" >nul\r\n",
    "@if errorlevel 1 exit /b 1\r\n",
    "@type \"%~3\"\r\n",
);

#[test]
fn independent_processes_keep_same_named_executables_and_inputs_private() -> Result<()> {
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/documentation-fixture-probes");
    Dir::create_ambient_dir_all(&fixture_root, ambient_authority())?;
    let state = tempfile::tempdir_in(fixture_root)?;
    std::thread::scope(|scope| {
        let first = scope.spawn(|| worker("first", state.path()));
        let second = scope.spawn(|| worker("second", state.path()));
        first
            .join()
            .map_err(|_| anyhow::anyhow!("first snapshot worker panicked"))??;
        second
            .join()
            .map_err(|_| anyhow::anyhow!("second snapshot worker panicked"))??;
        Ok(())
    })
}

fn worker(identity: &str, state: &Path) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--ignored", "--nocapture", "build_snapshot_probe"])
        .env("ORTHO_DOC_WORKER", identity)
        .env("ORTHO_DOC_SNAPSHOT_STATE", state);
    let output = process_runner::run_with_policy(
        &mut command,
        Operation("exercise process-private executable snapshot"),
        Policy::new(
            "execution",
            Duration::from_secs(10),
            Duration::from_secs(2),
            1024 * 1024,
        ),
    )?;
    ensure!(
        output.status.success(),
        "snapshot worker failed: {output:?}"
    );
    ensure!(String::from_utf8_lossy(&output.stdout).contains(&format!("SNAPSHOT_OK:{identity}")));
    Ok(())
}

#[test]
#[ignore = "executed as an independent build-and-snapshot process"]
fn build_snapshot_probe() -> Result<()> {
    let identity = std::env::var("ORTHO_DOC_WORKER").context("snapshot identity")?;
    ensure!(matches!(identity.as_str(), "first" | "second"));
    let state = std::env::var_os("ORTHO_DOC_SNAPSHOT_STATE").context("snapshot state")?;
    let state_path = Path::new(&state);
    let owner = tempfile::tempdir_in(state_path)?;
    let directory = Dir::open_ambient_dir(state_path, ambient_authority())?;
    let cache = BuildCache::for_parent_target(state_path);
    let inputs = Dir::open_ambient_dir(owner.path(), ambient_authority())?;
    inputs.write("compiler-output", script(&identity))?;
    inputs.write("compiler.cmd", COMPILER_SOURCE)?;
    let record = serde_json::json!({
        "reason": "compiler-artifact", "target": {"name": "same-name", "kind": ["bin"]},
        "executable": cache.root.join(SCRIPT_NAME),
    });
    inputs.write("compiler-record.json", format!("{record}\n"))?;
    let snapshots = cache.build_with(
        BuildRequest {
            package: owner.path(),
            destination: &owner.path().join("executables"),
            started: Instant::now(),
            timeout: Duration::from_secs(5),
        },
        |shared, _| Ok(compiler_command(owner.path(), &shared.join(SCRIPT_NAME))),
    )?;
    directory.write(format!("ready-{identity}"), b"private snapshot complete")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !(directory.exists("ready-first") && directory.exists("ready-second")) {
        ensure!(Instant::now() < deadline, "other snapshot did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
    let executable = snapshots.get("same-name").context("missing snapshot")?;
    for value in ["initial", "updated"] {
        inputs.write("value.txt", value)?;
        let mut command = script_command(executable);
        command.current_dir(owner.path());
        let output = process_runner::run_with_policy(
            &mut command,
            Operation("run same-named fixture snapshot"),
            Policy::new(
                "execution",
                Duration::from_secs(2),
                Duration::from_secs(2),
                1024 * 1024,
            ),
        )?;
        ensure!(output.status.success(), "fixture failed: {output:?}");
        ensure!(String::from_utf8(output.stdout)?.trim() == format!("{identity}:{value}"));
    }
    writeln!(std::io::stdout().lock(), "\nSNAPSHOT_OK:{identity}")?;
    Ok(())
}

#[cfg(unix)]
fn script(identity: &str) -> String {
    format!("read -r value < value.txt\nprintf '{identity}:%s' \"$value\"\n")
}

#[cfg(windows)]
fn script(identity: &str) -> String {
    format!("@echo off\r\nset /p value=<value.txt\r\necho {identity}:%value%\r\n")
}

#[cfg(unix)]
fn script_command(executable: &Path) -> Command {
    let mut command = Command::new("/bin/sh");
    command.arg(executable);
    command
}

fn compiler_command(inputs: &Path, shared_executable: &Path) -> Command {
    #[cfg(unix)]
    let mut command = Command::new("/bin/sh");
    #[cfg(windows)]
    let mut command = Command::new("cmd.exe");
    #[cfg(windows)]
    command.args(["/d", "/c"]);
    command
        .arg(inputs.join("compiler.cmd"))
        .arg(inputs.join("compiler-output"))
        .arg(shared_executable)
        .arg(inputs.join("compiler-record.json"));
    command
}

#[cfg(windows)]
fn script_command(executable: &Path) -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/d", "/c"]).arg(executable);
    command
}
