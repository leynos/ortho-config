//! Remove only the generated bridge target's Cargo-reported output files.
//!
//! The caller holds the production bridge build lock and has already copied
//! the executable into its private invocation directory. Removal is best
//! effort, but its outcome is evidence the caller reports rather than silence.

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::io;

use super::protocol::is_generated_output_name;
use super::telemetry;

/// What the caller established about generated-output cleanup.
///
/// The outcome keeps a target directory that could not be opened distinct
/// from a directory whose reported files were all removed. Collapsing the two
/// would let a total cleanup failure read as success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CleanupOutcome {
    /// Every reported output that belonged to this bridge is gone.
    Removed,
    /// The target directory could not be opened, so no output was examined.
    TargetUnreadable,
    /// At least one reported output survived removal.
    OutputsRemaining,
}

impl CleanupOutcome {
    /// Report whether cleanup removed every reported output it could reach.
    pub(super) const fn is_complete(self) -> bool {
        matches!(self, Self::Removed)
    }

    /// Name the outcome within the closed telemetry vocabulary.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::TargetUnreadable => "target_unreadable",
            Self::OutputsRemaining => "outputs_remaining",
        }
    }
}

/// Remove reported outputs after snapshotting and report what remained.
pub(super) fn remove_generated_outputs(
    target_directory: &Utf8Path,
    binary_name: &str,
    outputs: &[camino::Utf8PathBuf],
) -> CleanupOutcome {
    let Ok(target) = Dir::open_ambient_dir(target_directory, ambient_authority()) else {
        telemetry::cleanup_target_open_failed();
        return CleanupOutcome::TargetUnreadable;
    };

    let mut outcome = CleanupOutcome::Removed;
    for output in outputs {
        let Some(relative) = output.strip_prefix(target_directory).ok() else {
            telemetry::cleanup_output_outside_target();
            continue;
        };
        outcome = worst_of(
            outcome,
            remove_reported_output(&target, relative, binary_name),
        );
    }
    outcome
}

/// Retain the outcome that reports the most surviving evidence.
const fn worst_of(current: CleanupOutcome, next: CleanupOutcome) -> CleanupOutcome {
    match (current, next) {
        (CleanupOutcome::TargetUnreadable, _) | (_, CleanupOutcome::TargetUnreadable) => {
            CleanupOutcome::TargetUnreadable
        }
        (CleanupOutcome::OutputsRemaining, _) | (_, CleanupOutcome::OutputsRemaining) => {
            CleanupOutcome::OutputsRemaining
        }
        (CleanupOutcome::Removed, CleanupOutcome::Removed) => CleanupOutcome::Removed,
    }
}

/// Remove one reported output when its name identifies this bridge binary.
fn remove_reported_output(target: &Dir, relative: &Utf8Path, binary_name: &str) -> CleanupOutcome {
    let Some(file_name) = relative.file_name() else {
        return CleanupOutcome::Removed;
    };
    if !is_generated_output_name(file_name, binary_name) {
        return CleanupOutcome::Removed;
    }
    match target.remove_file(relative.as_str()) {
        Ok(()) => CleanupOutcome::Removed,
        // Another owner removed the file first; the postcondition still holds.
        Err(error) if error.kind() == io::ErrorKind::NotFound => CleanupOutcome::Removed,
        Err(_) => {
            telemetry::cleanup_remove_failed();
            CleanupOutcome::OutputsRemaining
        }
    }
}
