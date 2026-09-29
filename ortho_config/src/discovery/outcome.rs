//! Outcome container shared by configuration discovery operations.
//!
//! Captures a discovered value alongside partitioned required and optional
//! errors so callers can preserve provenance when aggregating failures.
use std::sync::Arc;

use crate::OrthoError;

/// Separates a discovered value from required and optional candidate failures.
///
/// Required errors remain fatal even when a later fallback succeeds; optional
/// errors are retained for diagnostics without changing discovery success.
#[derive(Debug, Default)]
pub(crate) struct DiscoveryOutcome<T> {
    /// Value produced by the first successful candidate, if one was found.
    pub(crate) value: Option<T>,
    /// Candidate errors that remain actionable even after a fallback succeeds.
    pub(crate) required_errors: Vec<Arc<OrthoError>>,
    /// Errors from optional candidates, available for aggregation after search.
    pub(crate) optional_errors: Vec<Arc<OrthoError>>,
}
