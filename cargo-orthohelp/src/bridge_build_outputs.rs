//! Remove only the generated bridge target's Cargo-reported output files.
//!
//! The caller holds the production bridge build lock and has already copied
//! the executable into its private invocation directory.

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use super::protocol::is_generated_output_name;

/// Remove reported outputs after snapshotting, warning without failing the IR build.
pub(super) fn remove_generated_outputs(
    target_directory: &Utf8Path,
    binary_name: &str,
    outputs: &[camino::Utf8PathBuf],
) {
    let Ok(target) = Dir::open_ambient_dir(target_directory, ambient_authority()) else {
        tracing::warn!(
            operation = "bridge_output_cleanup",
            outcome = "target_open_failed",
            "could not open Cargo target to remove generated bridge outputs"
        );
        return;
    };

    for output in outputs {
        let Some(relative) = output.strip_prefix(target_directory).ok() else {
            tracing::warn!(
                operation = "bridge_output_cleanup",
                outcome = "outside_target_ignored",
                "ignored generated bridge output outside its Cargo target"
            );
            continue;
        };
        let Some(file_name) = relative.file_name() else {
            continue;
        };
        if !is_generated_output_name(file_name, binary_name) {
            continue;
        }
        if let Err(error) = target.remove_file(relative.as_str())
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(
                operation = "bridge_output_cleanup",
                outcome = "remove_failed",
                "failed to remove generated bridge Cargo output after snapshot"
            );
        }
    }
}
