//! IR cache lookup with its tracing, telemetry and metrics.
//!
//! Split from the parent `bridge` module to keep it within the repository's
//! 400-line ceiling. A lookup either yields cached IR, reports a miss, or
//! reports a failed read, and records each outcome exactly once.

use std::time::Instant;

#[cfg(feature = "metrics")]
use super::telemetry_metrics;
use super::{BridgeCache, ir_cache::read_cached_ir, telemetry};
use crate::bridge_observability as observability;
use crate::error::OrthohelpError;

/// Read the cached IR, recording the lookup as a hit, a miss or a failed read.
pub(super) fn lookup_cached_ir(paths: &BridgeCache) -> Result<Option<String>, OrthohelpError> {
    let lookup_started = Instant::now();
    match read_cached_ir(paths) {
        Ok(Some(cached)) => {
            record_lookup_hit(lookup_started);
            Ok(Some(cached))
        }
        Ok(None) => {
            record_lookup_miss(lookup_started);
            Ok(None)
        }
        Err(error) => {
            record_lookup_failure(lookup_started);
            Err(error)
        }
    }
}

/// Record a cache hit in tracing, telemetry and metrics.
fn record_lookup_hit(started: Instant) {
    observability::completed("bridge_cache_lookup", "hit", started);
    telemetry::ir_cache_hit();
    #[cfg(feature = "metrics")]
    telemetry_metrics::record_cache_event(
        telemetry_metrics::CacheOperation::Lookup,
        telemetry_metrics::CacheOutcome::Hit,
    );
}

/// Record a cache miss in tracing, telemetry and metrics.
fn record_lookup_miss(started: Instant) {
    observability::completed("bridge_cache_lookup", "miss", started);
    telemetry::ir_cache_miss();
    #[cfg(feature = "metrics")]
    telemetry_metrics::record_cache_event(
        telemetry_metrics::CacheOperation::Lookup,
        telemetry_metrics::CacheOutcome::Miss,
    );
}

/// Record a failed cache read in tracing and metrics.
fn record_lookup_failure(started: Instant) {
    observability::failed("bridge_cache_lookup", "read_failed", started);
    #[cfg(feature = "metrics")]
    telemetry_metrics::record_cache_event(
        telemetry_metrics::CacheOperation::Lookup,
        telemetry_metrics::CacheOutcome::ReadFailed,
    );
}
