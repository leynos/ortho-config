//! Bounded labels and scopes carried while discovery candidates are assembled.
//!
//! Candidate generation assigns these values where each source is known. They
//! preserve telemetry decisions and automatic scope without retaining paths or
//! raw environment values; explicit paths and selectors have no automatic scope.

use super::super::DiscoveryScope;

/// The bounded XDG presence and resolution labels retained for later telemetry.
///
/// Presence labels distinguish absent, empty, and non-empty environment values
/// without retaining their contents. The resolution label records whether a
/// usable configured list supplied bases or the default branch was selected.
pub(super) struct XdgDecisions {
    /// `XDG_CONFIG_HOME` presence from the closed absent/empty/present vocabulary.
    pub(super) config_home: &'static str,
    /// `XDG_CONFIG_DIRS` presence from the closed absent/empty/present vocabulary.
    pub(super) dirs: &'static str,
    /// Configured-list or platform-default branch selected for system bases.
    pub(super) resolution: &'static str,
}

/// Bounded telemetry source and automatic scope assigned to generated candidates.
///
/// The source label comes from the closed candidate vocabulary. Scope records
/// whether the automatic base is user, system, or project configuration; the
/// same source label can apply to different scopes, as with XDG home and system
/// directories.
#[derive(Clone, Copy)]
pub(super) struct CandidateOrigin {
    /// Closed `CANDIDATE_*` label retained on the first deduplicated path.
    pub(super) source: &'static str,
    /// Automatic discovery scope carried with every generated path.
    pub(super) scope: DiscoveryScope,
}

impl CandidateOrigin {
    /// Pairs a bounded source label with the scope of the automatic base.
    ///
    /// Call sites supply both values together because only candidate generation
    /// knows which source and scope produced each base.
    /// For example, an XDG home base pairs its source label with `User` scope.
    pub(super) const fn new(source: &'static str, scope: DiscoveryScope) -> Self {
        Self { source, scope }
    }
}
