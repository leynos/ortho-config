//! Outcome container shared by configuration discovery operations.
//!
//! Captures a discovered value alongside partitioned required and optional
//! errors so callers can preserve provenance when aggregating failures.
use std::sync::Arc;

use crate::OrthoError;

#[derive(Debug, Default)]
pub(crate) struct DiscoveryOutcome<T> {
    pub(crate) value: Option<T>,
    pub(crate) required_errors: Vec<Arc<OrthoError>>,
    pub(crate) optional_errors: Vec<Arc<OrthoError>>,
}
