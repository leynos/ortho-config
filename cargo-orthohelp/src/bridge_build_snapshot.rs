//! Validate Cargo's bridge result and copy its executable to private ownership.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use cap_std::fs_utf8::OpenOptions;
use std::io;

use super::{
    CargoBuild, CargoStatus, GeneratedTarget, binary_file_name, io_error, message_error,
    open_directory,
};
use crate::error::OrthohelpError;

/// Validate Cargo's status and protocol state, then snapshot its binary.
pub(super) fn finish_build(
    build: CargoBuild,
    target: &GeneratedTarget<'_>,
    snapshot_directory: &Utf8Path,
    snapshot: &Dir,
) -> Result<Utf8PathBuf, OrthohelpError> {
    if let CargoStatus::Failure(status) = build.status {
        return Err(OrthohelpError::BridgeBuildFailure {
            status,
            message: "Cargo build failed; rendered diagnostics were forwarded to stderr".to_owned(),
        });
    }
    if let Some(error) = build.messages.protocol_error {
        return Err(error);
    }
    let executable = build.messages.executable.ok_or_else(|| {
        message_error(&format!(
            "Cargo build succeeded without a bin {} artifact",
            target.binary_name
        ))
    })?;
    let private_executable = snapshot_executable(&executable, target, snapshot_directory, snapshot)
        .map_err(|error| {
            tracing::warn!(
                target_directory = %target.directory,
                artifact = %executable,
                error = %error,
                "bridge snapshot failed after Cargo exited"
            );
            error
        })?;
    super::outputs::remove_generated_outputs(
        target.directory,
        target.binary_name,
        &build.messages.generated_outputs,
    );
    Ok(private_executable)
}

/// Copy Cargo's reported executable to a private invocation-owned directory.
fn snapshot_executable(
    executable: &Utf8Path,
    target: &GeneratedTarget<'_>,
    snapshot_directory: &Utf8Path,
    snapshot: &Dir,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let executable_name = binary_file_name(target.binary_name);
    if executable.file_name() != Some(executable_name.as_str()) {
        return Err(message_error(
            "Cargo artifact name did not match the generated bridge target",
        ));
    }
    let source_directory = executable
        .parent()
        .ok_or_else(|| message_error("Cargo artifact path had no parent directory"))?;
    let source = open_directory(source_directory)?;
    let source_file = source
        .open(executable_name.as_str())
        .map_err(|error| io_error(executable, error))?;
    let permissions = source_file
        .metadata()
        .map_err(|error| io_error(executable, error))?
        .permissions();
    match snapshot.symlink_metadata(executable_name.as_str()) {
        Ok(_) => {
            return Err(message_error(
                "private bridge snapshot already contained orthohelp_bridge",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(snapshot_directory, error)),
    }
    let destination_path = snapshot_directory.join(&executable_name);
    let mut destination = snapshot
        .open_with(
            executable_name.as_str(),
            OpenOptions::new().write(true).create_new(true),
        )
        .map_err(|error| io_error(&destination_path, error))?;
    let copied = io::copy(&mut &source_file, &mut destination)
        .and_then(|_| destination.set_permissions(permissions))
        .and_then(|()| destination.sync_all());
    if let Err(copy_error) = copied {
        let primary = io_error(&destination_path, copy_error);
        if let Err(cleanup) = snapshot.remove_file(executable_name.as_str()) {
            tracing::warn!(
                path = %destination_path,
                error = %cleanup,
                primary = %primary,
                "failed to remove incomplete bridge snapshot"
            );
        }
        return Err(primary);
    }
    Ok(destination_path)
}

#[cfg(test)]
#[path = "bridge_build_snapshot_tests.rs"]
mod tests;
