//! Own the Cargo bridge build and its executable snapshot.
//!
//! This module is restricted to the generated `orthohelp_bridge` binary. It
//! keeps Cargo's reported artifact stable until a private run copy is complete.

#[path = "bridge_build_cleanup.rs"]
mod cleanup;
#[path = "bridge_build_diagnostics.rs"]
mod diagnostics;
#[path = "bridge_build_outputs.rs"]
mod outputs;
#[path = "bridge_build_process.rs"]
mod process;
#[path = "bridge_build_protocol.rs"]
mod protocol;
#[path = "bridge_build_snapshot.rs"]
mod snapshot;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use fs4::FileExt;
use process::run_cargo;
use protocol::CargoMessages;
use std::io::{self, Write};
use std::process::Command;
use std::time::Instant;

use crate::bridge_observability as observability;
use crate::error::OrthohelpError;

/// Stable OS lock name shared by production bridge build callers.
const LOCK_FILE: &str = "orthohelp-bridge-build.lock";
/// Marker left behind when build ownership cannot be confirmed as complete.
const MARKER_FILE: &str = "orthohelp-bridge-build.in-progress";

/// Cargo output location and target identity for one generated bridge binary.
#[derive(Clone, Copy)]
pub(super) struct GeneratedTarget<'a> {
    /// Selected Cargo target directory shared by bridge invocations.
    pub(super) directory: &'a Utf8Path,
    /// Explicit generated bin name, distinct from ordinary workspace targets.
    pub(super) binary_name: &'a str,
}

/// Build the bridge with Cargo and copy the reported executable into a private
/// run directory.
///
/// The caller supplies a Cargo `build` command with JSON messages enabled and
/// stderr inherited by its supervisor. The target-directory lock covers both
/// the command and the snapshot, while Cargo itself retains ownership of its
/// shared registry and build cache. The caller must place `snapshot_directory`
/// under a private invocation directory; this helper creates it when absent.
#[tracing::instrument(skip_all, fields(operation = "bridge_build_and_snapshot"))]
pub(super) fn build_and_snapshot(
    command: &mut Command,
    target_directory: &Utf8Path,
    snapshot_directory: &Utf8Path,
    binary_name: &str,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let started = Instant::now();
    let result = build_and_snapshot_with(target_directory, snapshot_directory, binary_name, || {
        run_cargo(
            command,
            GeneratedTarget {
                directory: target_directory,
                binary_name,
            },
        )
    });
    match &result {
        Ok(_) => observability::completed("bridge_build_and_snapshot", "success", started),
        Err(_) => observability::failed("bridge_build_and_snapshot", "failure", started),
    }
    result
}

/// Acquire build ownership, run Cargo, and snapshot its reported executable.
fn build_and_snapshot_with(
    target_directory: &Utf8Path,
    snapshot_directory: &Utf8Path,
    binary_name: &str,
    run: impl FnOnce() -> Result<CargoBuild, CargoFailure>,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let started = Instant::now();
    let target = GeneratedTarget {
        directory: target_directory,
        binary_name,
    };
    let ownership = BuildOwnership::acquire(target.directory)?;
    refuse_stale_marker(&ownership.directory, target.directory)?;
    Dir::create_ambient_dir_all(snapshot_directory, ambient_authority())
        .map_err(|error| io_error(snapshot_directory, error))?;
    let snapshot = open_directory(snapshot_directory)?;
    let marker = BuildMarker::create(&ownership.directory, target.directory)?;

    let build = match run() {
        Ok(build) => build,
        Err(failure) => {
            if failure.writer_lifetime_confirmed {
                remove_marker_after_failure(&marker);
            } else {
                tracing::warn!(
                    operation = "bridge_build_and_snapshot",
                    outcome = "writer_lifetime_unconfirmed",
                    marker_retained = true,
                    "bridge child lifetime is unconfirmed; recovery marker retained"
                );
                observability::failed(
                    "bridge_cargo_cleanup",
                    "writer_lifetime_unconfirmed",
                    started,
                );
            }
            return Err(failure.error);
        }
    };

    let result = snapshot::finish_build(build, &target, snapshot_directory, &snapshot);
    match result {
        Ok(executable) => {
            marker.remove()?;
            Ok(executable)
        }
        Err(primary) => {
            remove_marker_after_failure(&marker);
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
    #[tracing::instrument(skip_all, fields(operation = "bridge_target_lock_acquire"))]
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
        let lock_started = Instant::now();
        if let Err(error) = FileExt::lock(&lock) {
            observability::failed("bridge_target_lock", "acquire_failed", lock_started);
            return Err(io_error(&lock_path, error));
        }
        observability::completed("bridge_target_lock", "acquired", lock_started);
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
            remove_marker_after_failure(&Self {
                directory,
                path: path.clone(),
            });
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

/// Open a directory with ambient authority and retain path context on failure.
fn open_directory(path: &Utf8Path) -> Result<Dir, OrthohelpError> {
    Dir::open_ambient_dir(path, ambient_authority()).map_err(|error| io_error(path, error))
}

/// Refuse to reuse a target carrying evidence of an interrupted prior owner.
fn refuse_stale_marker(directory: &Dir, target_directory: &Utf8Path) -> Result<(), OrthohelpError> {
    match directory.symlink_metadata(MARKER_FILE) {
        Ok(_) => {
            tracing::warn!(
                operation = "bridge_build_marker",
                outcome = "stale_marker_rejected",
                marker_retained = true,
                "bridge build requires owner recovery after an interrupted build"
            );
            Err(message_error(&format!(
                "bridge target requires owner recovery after an interrupted build: {}; confirm no Cargo or rustc process still uses the target directory, then remove the marker and rerun",
                target_directory.join(MARKER_FILE)
            )))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&target_directory.join(MARKER_FILE), error)),
    }
}

/// Try to remove a recovery marker without replacing the primary build error.
fn remove_marker_after_failure(marker: &BuildMarker<'_>) {
    if marker.remove().is_err() {
        tracing::warn!(
            operation = "bridge_build_marker_cleanup",
            outcome = "remove_failed",
            marker_retained = true,
            "failed to remove bridge recovery marker after a returned error"
        );
    }
}

/// Record bounded-cleanup evidence while returning the original build failure.
fn with_cleanup_evidence(primary: OrthohelpError, cleanup: &str) -> OrthohelpError {
    tracing::warn!(
        operation = "bridge_child_cleanup",
        outcome = "writer_lifetime_unconfirmed",
        cleanup_evidence_present = !cleanup.is_empty(),
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

#[cfg(test)]
#[path = "bridge_build_tests.rs"]
mod tests;
