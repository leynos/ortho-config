//! The assembled candidate list and the decisions taken while assembling it.
//!
//! Split from `candidates` so the assembly logic and the value types it
//! produces each stay within the repository's 400-line module ceiling.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::ConfigDiscovery;
use super::telemetry;

/// Normalizes a path according to Windows' case-insensitive comparison rules by
/// lowercasing ASCII code points on the original wide path representation and
/// replacing forward slashes with backslashes.
///
/// The key stays a raw `Vec<u16>`: converting to `String` would map every
/// unpaired surrogate to U+FFFD, so two distinct native paths could share a
/// key and deduplication would silently drop a valid candidate.
#[cfg(windows)]
fn windows_normalized_key(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .map(|unit| match unit {
            65..=90 => unit + 32,
            47 => 92,
            _ => unit,
        })
        .collect()
}

impl ConfigDiscovery {
    /// Builds a native deduplication key without converting paths to UTF-8.
    pub(super) fn dedup_key(path: &Path) -> DedupKey {
        #[cfg(windows)]
        {
            windows_normalized_key(path)
        }

        #[cfg(not(windows))]
        {
            // The native `OsString`, not a lossy `String`: lossy conversion
            // maps every invalid byte to U+FFFD, so two distinct non-UTF-8
            // paths would collide and discovery would silently drop one.
            path.as_os_str().to_os_string()
        }
    }

    /// Exposes the production Windows key normalization to platform tests.
    #[cfg(all(test, windows))]
    pub(super) fn normalized_key(path: &Path) -> DedupKey {
        Self::dedup_key(path)
    }
}

/// Platform-shaped deduplication key.
///
/// Windows normalizes to the raw UTF-16 units (see `windows_normalized_key`);
/// every other platform keys on the native `OsString`. Both shapes preserve
/// paths that are not valid Unicode, so no two distinct paths share a key.
#[cfg(windows)]
pub(super) type DedupKey = Vec<u16>;
/// Native path representation used to distinguish non-UTF-8 paths off Windows.
///
/// Unlike a lossy `String`, an `OsString` keeps distinct native byte sequences
/// distinct during candidate deduplication.
#[cfg(not(windows))]
pub(super) type DedupKey = std::ffi::OsString;

/// One candidate path with the bounded label of the rung that produced it.
///
/// The label feeds candidate-failure telemetry. It is a `&'static str` drawn
/// from the closed `CANDIDATE_*` set in [`telemetry`], never a path, so the
/// module's no-values-in-events property survives the extra field.
pub(super) struct Candidate {
    /// Filesystem location attempted at this position in discovery order.
    pub(super) path: PathBuf,
    /// Bounded source label used to classify failures without exposing the path.
    pub(super) source: &'static str,
}

/// Accumulates candidates while deduplicating per the platform's path rules.
#[derive(Default)]
pub(super) struct CandidateAccumulator {
    /// Accepted paths in first-seen order; later duplicates never replace them.
    pub(super) candidates: Vec<Candidate>,
    /// Platform-native keys used to reject duplicate paths without lossy conversion.
    seen: HashSet<DedupKey>,
}

impl CandidateAccumulator {
    /// Appends a non-empty path only if its platform-specific key is new.
    ///
    /// Returning `false` for empty and duplicate paths keeps both candidate
    /// order and the required-prefix count aligned with paths that can actually
    /// be attempted.
    pub(super) fn push_unique(&mut self, candidate: PathBuf, source: &'static str) -> bool {
        if candidate.as_os_str().is_empty() {
            return false;
        }
        let key = ConfigDiscovery::dedup_key(&candidate);
        if self.seen.insert(key) {
            self.candidates.push(Candidate {
                path: candidate,
                source,
            });
            true
        } else {
            false
        }
    }
}

/// The decisions made while assembling the candidate list.
///
/// Assembly records its decisions instead of emitting them so that
/// [`ConfigDiscovery::candidates`] stays a silent query; discovery operations
/// call [`CandidateDecisions::emit`] at their own boundary, which is where a
/// side effect belongs.
pub(super) struct CandidateDecisions {
    /// Resolution state for the optional configuration-path selector.
    pub(super) selector: &'static str,
    /// Whether `XDG_CONFIG_HOME` was absent, empty, or usable.
    pub(super) xdg_config_home: &'static str,
    /// Whether `XDG_CONFIG_DIRS` was absent, empty, or usable.
    pub(super) xdg_dirs: &'static str,
    /// Whether the default XDG base or the configured list supplied candidates.
    pub(super) xdg_resolution: &'static str,
    /// Which source, if any, supplied the home directory.
    pub(super) home: &'static str,
}

impl CandidateDecisions {
    /// Emit the recorded decisions as the usual discovery telemetry events.
    pub(super) fn emit(&self) {
        telemetry::selector_decision(self.selector);
        telemetry::xdg_decision(self.xdg_config_home, self.xdg_dirs, self.xdg_resolution);
        telemetry::home_decision(self.home);
    }
}

/// The assembled candidate list, its required prefix, and the decisions taken.
pub(super) struct CandidateSet {
    /// Deduplicated paths in precedence order, with required paths first.
    pub(super) candidates: Vec<Candidate>,
    /// Length of the leading required-candidate prefix in `candidates`.
    pub(super) required_bound: usize,
    /// Decisions retained for emission by the load operation boundary.
    pub(super) decisions: CandidateDecisions,
}
