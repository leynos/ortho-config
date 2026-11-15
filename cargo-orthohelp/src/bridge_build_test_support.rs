//! Controlled child-process fixtures for bridge-build ownership tests.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::fs_utf8::{Dir, OpenOptions};
use serde_json::json;
use std::io::{Cursor, Read, Write};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[path = "bridge_build_test_process.rs"]
mod process;

pub(super) use process::ProbeProcess;

const PROBE_TARGET: &str = "ORTHOHELP_BRIDGE_PROBE_TARGET";
const PROBE_OWNER: &str = "ORTHOHELP_BRIDGE_PROBE_OWNER";
const PROBE_COORDINATION: &str = "ORTHOHELP_BRIDGE_PROBE_COORDINATION";
const PROBE_SNAPSHOT: &str = "ORTHOHELP_BRIDGE_PROBE_SNAPSHOT";
const PROBE_WAIT: Duration = Duration::from_secs(8);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const OVERSIZED_CARGO_RECORD_CHUNK: [u8; 8192] = [b'x'; 8192];
const FIXTURE_BINARY_NAME: &str = "orthohelp_bridge_fixture";

#[derive(Clone, Copy)]
struct FixtureOptions<'a> {
    binary_name: &'a str,
    owner: Option<&'a str>,
    coordination: Option<&'a Utf8Path>,
}

/// Cargo output paths owned by one controlled bridge artifact fixture.
struct FixtureArtifactPaths {
    executable: Utf8PathBuf,
    dependency_output: Utf8PathBuf,
    dep_info: Utf8PathBuf,
}

pub(super) fn binary_file_name(binary_name: &str) -> String {
    super::super::protocol::binary_file_name(binary_name)
}

pub(super) fn utf8_path(path: std::path::PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|invalid_path| {
        anyhow::anyhow!("temporary path was not UTF-8: {}", invalid_path.display())
    })
}

pub(super) fn create_directory_all(path: &Utf8Path) -> Result<()> {
    Dir::create_ambient_dir_all(path, ambient_authority())?;
    Ok(())
}

pub(super) fn write_bytes(path: &Utf8Path, bytes: &[u8]) -> Result<()> {
    let (parent, name) = path_parts(path)?;
    create_directory_all(parent)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    let mut file = directory.open_with(
        name,
        OpenOptions::new().write(true).create(true).truncate(true),
    )?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(super) fn read_text(path: &Utf8Path) -> Result<String> {
    let (parent, name) = path_parts(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    let mut file = directory.open(name)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub(super) fn path_exists(path: &Utf8Path) -> Result<bool> {
    let (parent, name) = path_parts(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok(directory.try_exists(name)?)
}

fn path_parts(path: &Utf8Path) -> Result<(&Utf8Path, &str)> {
    let parent = path.parent().context("finding test path parent")?;
    let name = path.file_name().context("finding test path file name")?;
    Ok((parent, name))
}

pub(super) fn build_probe(
    mode: &str,
    target: &Utf8Path,
    snapshot: &Utf8Path,
    binary_name: &str,
) -> Result<Utf8PathBuf> {
    Ok(super::super::build_and_snapshot_with(
        target,
        snapshot,
        binary_name,
        || {
            fixture_cargo_build(
                mode,
                target,
                FixtureOptions {
                    binary_name,
                    owner: None,
                    coordination: None,
                },
            )
        },
    )?)
}

pub(super) fn owner_process(
    owner: &str,
    target: &Utf8Path,
    coordination: &Utf8Path,
    snapshot: &Utf8Path,
) -> Result<Command> {
    let mut command = test_process("cross_process_owner_probe")?;
    command
        .env(PROBE_OWNER, owner)
        .env(PROBE_TARGET, target.as_str())
        .env(PROBE_COORDINATION, coordination.as_str())
        .env(PROBE_SNAPSHOT, snapshot.as_str());
    Ok(command)
}

fn test_process(filter: &str) -> Result<Command> {
    let executable = std::env::current_exe().context("locating the current test executable")?;
    let mut command = Command::new(executable);
    command.args(["--ignored", "--quiet", "--test-threads=1", filter]);
    Ok(command)
}

fn fixture_cargo_build(
    mode: &str,
    target: &Utf8Path,
    options: FixtureOptions<'_>,
) -> std::result::Result<super::super::CargoBuild, super::super::CargoFailure> {
    let messages = fixture_cargo_messages(mode, target, options)
        .map_err(|error| confirmed_fixture_failure(&error))?;
    Ok(super::super::CargoBuild {
        status: super::super::CargoStatus::Success,
        messages,
    })
}

fn fixture_cargo_messages(
    mode: &str,
    target: &Utf8Path,
    options: FixtureOptions<'_>,
) -> Result<super::super::protocol::CargoMessages> {
    let paths = fixture_artifact_paths(target, options)?;
    let stream = fixture_cargo_stream(mode, options, &paths)?;
    let mut reader = Cursor::new(stream);
    let generated_target = super::super::GeneratedTarget {
        directory: target,
        binary_name: options.binary_name,
    };
    Ok(super::super::protocol::read_cargo_messages(
        &mut reader,
        &generated_target,
    )?)
}

/// Derive the private paths used by one controlled Cargo artifact fixture.
fn fixture_artifact_paths(
    target: &Utf8Path,
    options: FixtureOptions<'_>,
) -> Result<FixtureArtifactPaths> {
    let executable = target
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(binary_file_name(options.binary_name));
    let dependency_output = executable
        .parent()
        .context("finding fake artifact parent")?
        .join("deps")
        .join(format!("{}-fixture-hash", options.binary_name));
    let dep_info = executable.with_extension("d");
    Ok(FixtureArtifactPaths {
        executable,
        dependency_output,
        dep_info,
    })
}

/// Produce malformed, oversized, or successful Cargo JSON for a probe.
fn fixture_cargo_stream(
    mode: &str,
    options: FixtureOptions<'_>,
    paths: &FixtureArtifactPaths,
) -> Result<Vec<u8>> {
    let mut stream = Vec::new();
    if mode == "malformed" {
        stream.extend_from_slice(b"not-json\n");
    } else if mode == "oversized" {
        for _ in 0..16 {
            stream.extend_from_slice(&OVERSIZED_CARGO_RECORD_CHUNK);
        }
        stream.extend_from_slice(b"x\n");
    } else {
        let identity = options.owner.unwrap_or("bridge fixture");
        create_directory_all(
            paths
                .executable
                .parent()
                .context("finding fake artifact parent")?,
        )?;
        write_bytes(&paths.executable, identity.as_bytes())?;
        if mode == "cleanup-failure" {
            create_directory_all(&paths.dependency_output)?;
        } else {
            write_bytes(&paths.dependency_output, b"generated compiler output")?;
        }
        write_bytes(&paths.dep_info, b"generated dep-info")?;
        #[cfg(unix)]
        set_executable(&paths.executable)?;

        if mode == "owned" {
            publish_owner_started(
                identity,
                options.coordination.context("owner coordination path")?,
            )?;
        }
        append_artifact_records(
            &mut stream,
            &paths.executable,
            &[&paths.dependency_output, &paths.dep_info],
            options.binary_name,
        )?;
        if mode == "overlap" {
            let ordinary_binary = paths
                .executable
                .parent()
                .context("finding fake artifact parent")?
                .join(binary_file_name("orthohelp_bridge"));
            write_bytes(&ordinary_binary, b"ordinary workspace binary")?;
        }
    }
    Ok(stream)
}

fn confirmed_fixture_failure(error: &anyhow::Error) -> super::super::CargoFailure {
    super::super::CargoFailure::confirmed(crate::error::OrthohelpError::Message(format!(
        "controlled Cargo fixture failed: {error:#}"
    )))
}

fn publish_owner_started(owner: &str, coordination: &Utf8Path) -> Result<()> {
    write_bytes(&coordination.join(format!("started-{owner}")), b"started")?;
    if owner == "A" {
        wait_for_file(&coordination.join("release-A"))?;
    }
    Ok(())
}

fn append_artifact_records(
    stream: &mut Vec<u8>,
    executable: &Utf8Path,
    generated_outputs: &[&Utf8Path],
    binary_name: &str,
) -> Result<()> {
    let mut filenames = vec![executable.as_str()];
    filenames.extend(generated_outputs.iter().map(|path| path.as_str()));
    for (name, kind, executable_path) in [
        ("fixture_dependency", "lib", None),
        (binary_name, "lib", None),
        (binary_name, "bin", Some(executable.as_str())),
    ] {
        let record = json!({
            "reason": "compiler-artifact",
            "target": { "name": name, "kind": [kind] },
            "filenames": if kind == "bin" { filenames.clone() } else { Vec::new() },
            "executable": executable_path
        });
        serde_json::to_writer(&mut *stream, &record)?;
        stream.push(b'\n');
    }
    Ok(())
}

pub(super) fn run_owner_process() -> Result<()> {
    let owner = std::env::var(PROBE_OWNER).context("reading bridge owner identity")?;
    let target = env_path(PROBE_TARGET)?;
    let coordination = env_path(PROBE_COORDINATION)?;
    let snapshot = env_path(PROBE_SNAPSHOT)?;
    if owner == "B" {
        confirm_other_process_holds_lock(&target)?;
        write_bytes(&coordination.join("attempted-B"), b"attempted")?;
    }
    let executable =
        super::super::build_and_snapshot_with(&target, &snapshot, FIXTURE_BINARY_NAME, || {
            fixture_cargo_build(
                "owned",
                &target,
                FixtureOptions {
                    binary_name: FIXTURE_BINARY_NAME,
                    owner: Some(&owner),
                    coordination: Some(&coordination),
                },
            )
        })?;
    let content = read_text(&executable)?;
    ensure!(
        content == owner,
        "snapshot was overwritten by a competing process"
    );
    Ok(())
}

fn confirm_other_process_holds_lock(target: &Utf8Path) -> Result<()> {
    use fs4::{FileExt, TryLockError};

    let directory = Dir::open_ambient_dir(target, ambient_authority())?;
    let lock = directory
        .open_with(
            super::super::LOCK_FILE,
            OpenOptions::new().read(true).write(true),
        )?
        .into_std();
    match FileExt::try_lock(&lock) {
        Err(TryLockError::WouldBlock) => Ok(()),
        Err(TryLockError::Error(error)) => Err(error).context("checking cross-process lock"),
        Ok(()) => {
            FileExt::unlock(&lock)?;
            anyhow::bail!("first owner did not hold the bridge target lock")
        }
    }
}

pub(super) fn wait_for_file(path: &Utf8Path) -> Result<()> {
    let deadline = Instant::now() + PROBE_WAIT;
    while Instant::now() < deadline {
        if path_exists(path)? {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    anyhow::bail!("timed out waiting for controlled probe file {path}")
}

pub(super) fn release_owner_on_drop(coordination: &Utf8Path) -> ReleaseOwner {
    ReleaseOwner {
        marker: coordination.join("release-A"),
    }
}

pub(super) struct ReleaseOwner {
    marker: Utf8PathBuf,
}

impl Drop for ReleaseOwner {
    fn drop(&mut self) {
        if let Err(error) = write_bytes(&self.marker, b"release") {
            tracing::warn!(path = %self.marker, error = %error, "failed to release test owner");
        }
    }
}

#[cfg(unix)]
pub(super) fn ensure_executable(path: &Utf8Path) -> Result<()> {
    let (parent, name) = path_parts(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    let mode = directory.open(name)?.metadata()?.permissions().mode();
    ensure!(
        mode & 0o111 != 0,
        "snapshotted bridge lost execute permissions"
    );
    Ok(())
}

fn env_path(name: &str) -> Result<Utf8PathBuf> {
    let raw = std::env::var_os(name).with_context(|| format!("reading {name}"))?;
    let path = std::path::PathBuf::from(raw);
    utf8_path(path)
}

#[cfg(unix)]
fn set_executable(path: &Utf8Path) -> Result<()> {
    let (parent, name) = path_parts(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    let file = directory.open(name)?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_mode(0o755);
    file.set_permissions(permissions)?;
    Ok(())
}
