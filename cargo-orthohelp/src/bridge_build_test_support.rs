//! Controlled child-process fixtures for bridge-build ownership tests.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::fs_utf8::{Dir, OpenOptions};
use serde_json::json;
use std::io::{Cursor, Read, Write};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const PROBE_TARGET: &str = "ORTHOHELP_BRIDGE_PROBE_TARGET";
const PROBE_OWNER: &str = "ORTHOHELP_BRIDGE_PROBE_OWNER";
const PROBE_COORDINATION: &str = "ORTHOHELP_BRIDGE_PROBE_COORDINATION";
const PROBE_SNAPSHOT: &str = "ORTHOHELP_BRIDGE_PROBE_SNAPSHOT";
const PROBE_WAIT: Duration = Duration::from_secs(8);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const OVERSIZED_CARGO_RECORD_CHUNK: [u8; 8192] = [b'x'; 8192];

pub(super) fn binary_file_name() -> String {
    super::super::binary_file_name()
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
) -> Result<Utf8PathBuf> {
    Ok(super::super::build_and_snapshot_with(
        target,
        snapshot,
        || fixture_cargo_build(mode, target, None, None),
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
    owner: Option<&str>,
    coordination: Option<&Utf8Path>,
) -> std::result::Result<super::super::CargoBuild, super::super::CargoFailure> {
    let messages = fixture_cargo_messages(mode, target, owner, coordination)
        .map_err(|error| confirmed_fixture_failure(&error))?;
    Ok(super::super::CargoBuild {
        status: super::super::CargoStatus::Success,
        messages,
    })
}

fn fixture_cargo_messages(
    mode: &str,
    target: &Utf8Path,
    owner: Option<&str>,
    coordination: Option<&Utf8Path>,
) -> Result<super::super::protocol::CargoMessages> {
    let executable = target
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(binary_file_name());
    let mut stream = Vec::new();
    if mode == "malformed" {
        stream.extend_from_slice(b"not-json\n");
    } else if mode == "oversized" {
        for _ in 0..16 {
            stream.extend_from_slice(&OVERSIZED_CARGO_RECORD_CHUNK);
        }
        stream.extend_from_slice(b"x\n");
    } else {
        let identity = owner.unwrap_or("bridge fixture");
        create_directory_all(
            executable
                .parent()
                .context("finding fake artifact parent")?,
        )?;
        write_bytes(&executable, identity.as_bytes())?;
        set_executable(&executable)?;

        if mode == "owned" {
            publish_owner_started(identity, coordination.context("owner coordination path")?)?;
        }
        append_artifact_records(&mut stream, &executable)?;
    }

    let mut reader = Cursor::new(stream);
    Ok(super::super::protocol::read_cargo_messages(
        &mut reader,
        target,
    )?)
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

fn append_artifact_records(stream: &mut Vec<u8>, executable: &Utf8Path) -> Result<()> {
    for (name, kind, executable_path) in [
        ("fixture_dependency", "lib", None),
        ("orthohelp_bridge", "lib", None),
        ("orthohelp_bridge", "bin", Some(executable.as_str())),
    ] {
        let record = json!({
            "reason": "compiler-artifact",
            "target": { "name": name, "kind": [kind] },
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
    let executable = super::super::build_and_snapshot_with(&target, &snapshot, || {
        fixture_cargo_build("owned", &target, Some(&owner), Some(&coordination))
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

pub(super) struct ProbeProcess(Option<Child>);

impl ProbeProcess {
    pub(super) fn spawn(command: &mut Command) -> Result<Self> {
        Ok(Self(Some(command.spawn()?)))
    }

    pub(super) fn wait(mut self) -> Result<()> {
        let mut child = self
            .0
            .take()
            .context("controlled probe process was already reaped")?;
        wait_for_success(&mut child)
    }
}

impl Drop for ProbeProcess {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            kill_probe(child);
            reap_after_kill(child);
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

#[cfg(not(unix))]
pub(super) fn ensure_executable(_path: &Utf8Path) -> Result<()> {
    Ok(())
}

pub(super) fn wait_for_success(child: &mut Child) -> Result<()> {
    let deadline = Instant::now() + PROBE_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                ensure!(status.success(), "probe process exited with {status}");
                return Ok(());
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                kill_probe(child);
                reap_after_kill(child);
                anyhow::bail!("controlled owner process exceeded its bounded wait")
            }
            Err(error) => {
                kill_probe(child);
                reap_after_kill(child);
                return Err(error).context("waiting for controlled owner process");
            }
        }
    }
}

fn reap_after_kill(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                tracing::warn!(error = %error, "failed to reap controlled test probe");
                break;
            }
        }
    }
}

fn kill_probe(child: &mut Child) {
    if let Err(error) = child.kill() {
        tracing::warn!(error = %error, "failed to terminate controlled test probe");
    }
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

#[cfg(not(unix))]
fn set_executable(_path: &Utf8Path) -> Result<()> {
    Ok(())
}
