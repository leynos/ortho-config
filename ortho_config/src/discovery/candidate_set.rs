//! The assembled candidate list and the decisions taken while assembling it.
//!
//! Split from `candidates` so the assembly logic and the value types it
//! produces each stay within the repository's 400-line module ceiling.

use std::collections::HashSet;
use std::path::PathBuf;

use super::ConfigDiscovery;
use super::telemetry;

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
    seen: HashSet<String>,
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
