//! Optional, bounded metrics for bridge IR cache outcomes.
//!
//! This module is owned by the bridge cache boundary and is called only from
//! `bridge/mod.rs`. The closed enums keep metric labels low-cardinality; paths,
//! keys, and error text remain in neither the metric name nor its labels.

/// Bridge cache operation used as the metric's `operation` label.
#[derive(Clone, Copy)]
pub(super) enum CacheOperation {
    /// Read cached IR before deciding whether a bridge build is needed.
    Lookup,
    /// Publish newly generated IR to the cache.
    Publish,
}

impl CacheOperation {
    /// Return the stable, bounded metric label for this operation.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Lookup => "lookup",
            Self::Publish => "publish",
        }
    }
}

/// Terminal bridge cache result used as the metric's `outcome` label.
#[derive(Clone, Copy)]
pub(super) enum CacheOutcome {
    /// A lookup returned existing cached IR.
    Hit,
    /// A lookup found no cached IR.
    Miss,
    /// Reading the existing cache entry failed.
    ReadFailed,
    /// Publishing was skipped because the cache already held identical IR.
    Unchanged,
    /// Newly generated IR was published successfully.
    Updated,
    /// Creating, writing, or replacing the cache entry failed.
    WriteFailed,
}

impl CacheOutcome {
    /// Return the stable, bounded metric label for this outcome.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::ReadFailed => "read_failed",
            Self::Unchanged => "unchanged",
            Self::Updated => "updated",
            Self::WriteFailed => "write_failed",
        }
    }
}

/// Increment one bridge cache event using only the closed label vocabularies.
pub(super) fn record_cache_event(operation: CacheOperation, outcome: CacheOutcome) {
    metrics::describe_counter!(
        "cargo_orthohelp.bridge_cache.events",
        "Bridge IR cache lookup and publication outcomes."
    );
    metrics::counter!(
        "cargo_orthohelp.bridge_cache.events",
        "operation" => operation.as_str(),
        "outcome" => outcome.as_str(),
    )
    .increment(1);
}
