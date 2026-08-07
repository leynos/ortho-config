//! The assembled candidate list and the decisions taken while assembling it.
//!
//! Split from `candidates` so the assembly logic and the value types it
//! produces each stay within the repository's 400-line module ceiling.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::ConfigDiscovery;
use super::telemetry;

#[cfg(windows)]
/// Normalizes a path according to Windows' case-insensitive comparison rules by
/// lowercasing ASCII code points on the original wide path representation and
/// replacing forward slashes with backslashes.
fn windows_normalized_key(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;

    let normalized: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .map(|unit| match unit {
            65..=90 => unit + 32,
            47 => 92,
            _ => unit,
        })
        .collect();
    String::from_utf16_lossy(&normalized)
}

impl ConfigDiscovery {
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

    #[cfg(all(test, windows))]
    pub(super) fn normalized_key(path: &Path) -> DedupKey {
        Self::dedup_key(path)
    }
}

/// Platform-shaped deduplication key.
///
/// Windows normalizes to a `String` (see `windows_normalized_key`); every
/// other platform keys on the native `OsString` so non-UTF-8 paths stay
/// distinct.
#[cfg(windows)]
pub(super) type DedupKey = String;
#[cfg(not(windows))]
pub(super) type DedupKey = std::ffi::OsString;

/// One candidate path with the bounded label of the rung that produced it.
///
/// The label feeds candidate-failure telemetry. It is a `&'static str` drawn
/// from the closed `CANDIDATE_*` set in [`telemetry`], never a path, so the
/// module's no-values-in-events property survives the extra field.
pub(super) struct Candidate {
    pub(super) path: PathBuf,
    pub(super) source: &'static str,
}

/// Accumulates candidates while deduplicating per the platform's path rules.
#[derive(Default)]
pub(super) struct CandidateAccumulator {
    pub(super) candidates: Vec<Candidate>,
    seen: HashSet<DedupKey>,
}

impl CandidateAccumulator {
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
    pub(super) selector: &'static str,
    pub(super) xdg_config_home: &'static str,
    pub(super) xdg_dirs: &'static str,
    pub(super) xdg_resolution: &'static str,
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
    pub(super) candidates: Vec<Candidate>,
    pub(super) required_bound: usize,
    pub(super) decisions: CandidateDecisions,
}
