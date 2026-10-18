//! Own the Cargo bridge build and its executable snapshot.
//!
//! This module is restricted to the generated `orthohelp_bridge` binary. It
//! keeps Cargo's reported artifact stable until a private run copy is complete.

#[path = "bridge_build_cleanup.rs"]
mod cleanup;
#[path = "bridge_build_protocol.rs"]
mod protocol;
#[path = "bridge_build_snapshot.rs"]
mod snapshot;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use fs4::FileExt;
use protocol::{CargoMessages, read_cargo_messages};
use std::io::{self, BufReader, Write};
use std::process::{Command, Stdio};

use crate::error::OrthohelpError;
use cleanup::OwnedChild;

/// Stable OS lock name shared by production bridge build callers.
const LOCK_FILE: &str = "orthohelp-bridge-build.lock";
/// Marker left behind when build ownership cannot be confirmed as complete.
const MARKER_FILE: &str = "orthohelp-bridge-build.in-progress";
/// Cargo target name whose reported executable is snapshotted.
const BRIDGE_BINARY_NAME: &str = "orthohelp_bridge";

/// Build the bridge with Cargo and copy the reported executable into a private
/// run directory.
///
/// The caller supplies a Cargo `build` command with JSON messages enabled and
/// stderr inherited by its supervisor. The target-directory lock covers both
/// the command and the snapshot, while Cargo itself retains ownership of its
/// shared registry and build cache. The caller must place `snapshot_directory`
/// under a private invocation directory; this helper creates it when absent.
pub(super) fn build_and_snapshot(
    command: &mut Command,
    target_directory: &Utf8Path,
    snapshot_directory: &Utf8Path,
) -> Result<Utf8PathBuf, OrthohelpError> {
    build_and_snapshot_with(target_directory, snapshot_directory, || {
        run_cargo(command, target_directory)
    })
}

/// Acquire build ownership, run Cargo, and snapshot its reported executable.
fn build_and_snapshot_with(
    target_directory: &Utf8Path,
    snapshot_directory: &Utf8Path,
    run: impl FnOnce() -> Result<CargoBuild, CargoFailure>,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let ownership = BuildOwnership::acquire(target_directory)?;
    refuse_stale_marker(&ownership.directory, target_directory)?;
    Dir::create_ambient_dir_all(snapshot_directory, ambient_authority())
        .map_err(|error| io_error(snapshot_directory, error))?;
    let snapshot = open_directory(snapshot_directory)?;
    let marker = BuildMarker::create(&ownership.directory, target_directory)?;

    let build = match run() {
        Ok(build) => build,
        Err(failure) => {
            if failure.writer_lifetime_confirmed {
                remove_marker_after_failure(&marker, &failure.error);
            } else {
                tracing::warn!(
                    target_directory = %target_directory,
                    error = %failure.error,
                    "bridge child lifetime is unconfirmed; recovery marker retained"
                );
            }
            return Err(failure.error);
        }
    };

    let result = snapshot::finish_build(build, target_directory, snapshot_directory, &snapshot);
    match result {
        Ok(executable) => {
            marker.remove()?;
            Ok(executable)
        }
        Err(primary) => {
            remove_marker_after_failure(&marker, &primary);
            Err(primary)
        }
    }
}

/// Holds the OS-level build lock and its capability-limited directory handle.
struct BuildOwnership {
    /// Directory containing the lock and stale-owner marker.
    directory: Dir,
    /// Open lock file retained for the complete build-and-snapshot interval.
    _lock: std::fs::File,
}

impl BuildOwnership {
    /// Create the target directory and wait for its cross-process build lock.
    fn acquire(target_directory: &Utf8Path) -> Result<Self, OrthohelpError> {
        Dir::create_ambient_dir_all(target_directory, ambient_authority())
            .map_err(|error| io_error(target_directory, error))?;
        let directory = open_directory(target_directory)?;
        let lock_path = target_directory.join(LOCK_FILE);
        let lock = directory
            .open_with(
                LOCK_FILE,
                OpenOptions::new().read(true).write(true).create(true),
            )
            .map_err(|error| io_error(&lock_path, error))?
            .into_std();
        FileExt::lock(&lock).map_err(|error| io_error(&lock_path, error))?;
        Ok(Self {
            directory,
            _lock: lock,
        })
    }
}

/// Records an active build so an interrupted owner cannot be mistaken for success.
struct BuildMarker<'a> {
    /// Directory used to create and remove the stable marker file.
    directory: &'a Dir,
    /// Path retained for diagnostics if marker operations fail.
    path: Utf8PathBuf,
}

impl<'a> BuildMarker<'a> {
    /// Create and sync the marker after build ownership has been acquired.
    fn create(directory: &'a Dir, target_directory: &Utf8Path) -> Result<Self, OrthohelpError> {
        let path = target_directory.join(MARKER_FILE);
        let mut marker = directory
            .open_with(MARKER_FILE, OpenOptions::new().write(true).create_new(true))
            .map_err(|error| io_error(&path, error))?;
        let contents = format!(
            "bridge build and snapshot active; pid={}\n",
            std::process::id()
        );
        if let Err(error) = marker
            .write_all(contents.as_bytes())
            .and_then(|()| marker.sync_all())
        {
            let primary = io_error(&path, error);
            remove_marker_after_failure(
                &Self {
                    directory,
                    path: path.clone(),
                },
                &primary,
            );
            return Err(primary);
        }
        Ok(Self { directory, path })
    }

    /// Remove the active marker after the build-and-snapshot interval ends.
    fn remove(&self) -> Result<(), OrthohelpError> {
        self.directory
            .remove_file(MARKER_FILE)
            .map_err(|error| io_error(&self.path, error))
    }
}

/// Cargo's exit status and the bounded protocol records observed during its build.
struct CargoBuild {
    /// Whether Cargo completed successfully or returned a failure code.
    status: CargoStatus,
    /// Parsed diagnostics and executable metadata from Cargo's JSON stream.
    messages: CargoMessages,
}

/// Process result retained after Cargo's direct child has been reaped.
#[derive(Clone, Copy)]
enum CargoStatus {
    /// Cargo returned a successful exit status.
    Success,
    /// Cargo returned a non-success status code.
    Failure(i32),
}

/// Build error plus evidence about whether Cargo's writer lifetime is known.
struct CargoFailure {
    /// Primary error returned to the bridge caller.
    error: OrthohelpError,
    /// Whether owned Cargo and its output writer were observed to finish.
    writer_lifetime_confirmed: bool,
}

impl CargoFailure {
    /// Record a failure whose writer lifetime is known to have ended.
    const fn confirmed(error: OrthohelpError) -> Self {
        Self {
            error,
            writer_lifetime_confirmed: true,
        }
    }

    /// Record a failure where Cargo or a descendant may still hold output open.
    const fn unconfirmed(error: OrthohelpError) -> Self {
        Self {
            error,
            writer_lifetime_confirmed: false,
        }
    }
}

/// Run Cargo with a piped JSON stream and return its status and parsed messages.
fn run_cargo(
    command: &mut Command,
    target_directory: &Utf8Path,
) -> Result<CargoBuild, CargoFailure> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());
    let child = command
        .spawn()
        .map_err(|error| CargoFailure::confirmed(io_error(target_directory, error)))?;
    let mut owned_child = OwnedChild::new(child);
    let Some(stdout) = owned_child.child.stdout.take() else {
        let primary = message_error("Cargo build stdout was not piped");
        let cleanup = owned_child.stop_bounded();
        return Err(CargoFailure::unconfirmed(with_cleanup_evidence(
            primary, &cleanup,
        )));
    };

    let mut reader = BufReader::new(stdout);
    let messages = match read_cargo_messages(&mut reader, target_directory) {
        Ok(messages) => messages,
        Err(error) => {
            drop(reader);
            let cleanup = owned_child.stop_bounded();
            return Err(CargoFailure::unconfirmed(with_cleanup_evidence(
                io_error(target_directory, error),
                &cleanup,
            )));
        }
    };
    drop(reader);

    let status = match owned_child.wait() {
        Ok(status) => status,
        Err(error) => {
            let cleanup = owned_child.stop_bounded();
            return Err(CargoFailure::unconfirmed(with_cleanup_evidence(
                io_error(target_directory, error),
                &cleanup,
            )));
        }
    };

    let cargo_status = if status.success() {
        CargoStatus::Success
    } else {
        CargoStatus::Failure(status.code().unwrap_or(-1))
    };
    Ok(CargoBuild {
        status: cargo_status,
        messages,
    })
}

/// Open a directory with ambient authority and retain path context on failure.
fn open_directory(path: &Utf8Path) -> Result<Dir, OrthohelpError> {
    Dir::open_ambient_dir(path, ambient_authority()).map_err(|error| io_error(path, error))
}

/// Refuse to reuse a target carrying evidence of an interrupted prior owner.
fn refuse_stale_marker(directory: &Dir, target_directory: &Utf8Path) -> Result<(), OrthohelpError> {
    match directory.symlink_metadata(MARKER_FILE) {
        Ok(_) => Err(message_error(&format!(
            "bridge target requires owner recovery after an interrupted build: {}",
            target_directory.join(MARKER_FILE)
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&target_directory.join(MARKER_FILE), error)),
    }
}

/// Try to remove a recovery marker without replacing the primary build error.
fn remove_marker_after_failure(marker: &BuildMarker<'_>, primary: &OrthohelpError) {
    if let Err(cleanup) = marker.remove() {
        tracing::warn!(
            marker = %marker.path,
            error = %cleanup,
            primary = %primary,
            "failed to remove bridge recovery marker after a returned error"
        );
    }
}

/// Record bounded-cleanup evidence while returning the original build failure.
fn with_cleanup_evidence(primary: OrthohelpError, cleanup: &str) -> OrthohelpError {
    tracing::warn!(
        error = %primary,
        cleanup = %cleanup,
        "bridge child cleanup was bounded; writer lifetime remains unconfirmed"
    );
    primary
}

/// Preserve the affected path when converting an I/O failure to a domain error.
fn io_error(path: &Utf8Path, source: io::Error) -> OrthohelpError {
    OrthohelpError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Create a bridge-build domain error without attaching an unrelated path.
fn message_error(message: &str) -> OrthohelpError {
    OrthohelpError::Message(message.to_owned())
}

/// Return the bridge executable name with its host platform suffix.
fn binary_file_name() -> String {
    format!("{BRIDGE_BINARY_NAME}{}", std::env::consts::EXE_SUFFIX)
}

#[cfg(test)]
#[path = "bridge_build_tests.rs"]
mod tests;
