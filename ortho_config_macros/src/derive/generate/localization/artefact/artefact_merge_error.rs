//! The category of a failed fragment merge.
//!
//! The merge spans several filesystem operations and one parse, and the two
//! failure kinds call for different responses: a maintainer reading a report
//! needs to know whether the build hit a permissions or space problem, or
//! whether a fragment on disk is not the JSON this version writes. Reporting
//! both as one category makes a harness group incomparable incidents together.
//!
//! The category travels *with* the failure rather than being reconstructed
//! afterwards, so a caller has nothing to infer and no message to parse.

use std::fmt;

use super::artefact_report::ErrorCategory;

/// Describes why a fragment merge failed, keeping the category with the detail.
///
/// The detail is the same actionable text the compiler diagnostic carries: it
/// names the offending path and the operation that failed, because the merge
/// runs inside a macro and its message reaches the build log verbatim.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct MergeError {
    /// The category the failing step belongs to.
    pub(super) category: ErrorCategory,
    /// The actionable text naming the path and the failed operation.
    pub(super) detail: String,
}

impl MergeError {
    /// Builds a failure the filesystem caused.
    pub(super) const fn io(detail: String) -> Self {
        Self {
            category: ErrorCategory::Io,
            detail,
        }
    }

    /// Builds a failure caused by a fragment that is not valid JSON.
    pub(super) const fn serialization(detail: String) -> Self {
        Self {
            category: ErrorCategory::Serialization,
            detail,
        }
    }
}

impl fmt::Display for MergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}
