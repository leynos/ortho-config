//! The `metrics` counters discovery increments, behind the optional feature.
//!
//! Split from `telemetry`, which carries the events and the label vocabulary
//! they draw on: the emission helpers and the closed sets they name together
//! fill that module to the repository's ceiling. The counters are a separate
//! concern at a separate boundary — each one is `#[cfg(feature = "metrics")]`
//! with a no-op twin, so an event can be recorded with the feature off while
//! the label vocabulary stays shared.
//!
//! Every label is a `&'static str` drawn from `telemetry`'s closed sets, so a
//! counter here cannot carry a path, a variable value, or any other datum that
//! would make a consumer's time-series unbounded.

/// Counts one discovery attempt, labelled by `operation`.
#[cfg(feature = "metrics")]
pub(super) fn count_attempt(operation: &'static str) {
    metrics::counter!("ortho_config.discovery.attempts", "operation" => operation).increment(1);
}

/// No-op arm of [`count_attempt`] when the `metrics` feature is off.
#[cfg(not(feature = "metrics"))]
pub(super) const fn count_attempt(_operation: &'static str) {}

/// Counts one terminal outcome, labelled by `operation` and `outcome`.
#[cfg(feature = "metrics")]
pub(super) fn count_outcome(operation: &'static str, outcome: &'static str) {
    metrics::counter!(
        "ortho_config.discovery.outcomes",
        "operation" => operation,
        "outcome" => outcome,
    )
    .increment(1);
}

/// No-op arm of [`count_outcome`] when the `metrics` feature is off.
#[cfg(not(feature = "metrics"))]
pub(super) const fn count_outcome(_operation: &'static str, _outcome: &'static str) {}

/// Counts one rejected candidate, labelled by `operation` and failure kind.
#[cfg(feature = "metrics")]
pub(super) fn count_candidate_failure(
    operation: &'static str,
    source: &'static str,
    category: &'static str,
) {
    metrics::counter!(
        "ortho_config.discovery.candidate_failures",
        "operation" => operation,
        "source" => source,
        "category" => category,
    )
    .increment(1);
}

/// No-op arm of [`count_candidate_failure`] when `metrics` is off.
#[cfg(not(feature = "metrics"))]
pub(super) const fn count_candidate_failure(
    _operation: &'static str,
    _source: &'static str,
    _category: &'static str,
) {
}
