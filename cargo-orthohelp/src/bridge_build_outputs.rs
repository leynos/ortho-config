//! Remove only the generated bridge target's Cargo-reported output files.
//!
//! The caller holds the production bridge build lock and has already copied
//! the executable into its private invocation directory.

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use super::protocol::is_generated_output_name;
use super::telemetry;

/// Remove reported outputs after snapshotting, warning without failing the IR build.
pub(super) fn remove_generated_outputs(
    target_directory: &Utf8Path,
    binary_name: &str,
    outputs: &[camino::Utf8PathBuf],
) {
    let Ok(target) = Dir::open_ambient_dir(target_directory, ambient_authority()) else {
        telemetry::cleanup_target_open_failed();
        return;
    };

    for output in outputs {
        let Some(relative) = output.strip_prefix(target_directory).ok() else {
            telemetry::cleanup_output_outside_target();
            continue;
        };
        remove_reported_output(&target, relative, binary_name);
    }
}

/// Remove one reported output when its name identifies this bridge binary.
fn remove_reported_output(target: &Dir, relative: &Utf8Path, binary_name: &str) {
    let Some(file_name) = relative.file_name() else {
        return;
    };
    if !is_generated_output_name(file_name, binary_name) {
        return;
    }
    if let Err(error) = target.remove_file(relative.as_str())
        && error.kind() != std::io::ErrorKind::NotFound
    {
        telemetry::cleanup_remove_failed();
    }
}
