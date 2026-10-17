//! One `tracing` call per event spelling.
//!
//! Split from `telemetry` for two reasons that reinforce each other.
//!
//! The first is size: the vocabulary, the public entrypoints and their
//! documentation together reach the repository's file ceiling, the same
//! pressure that produced `telemetry_counters`.
//!
//! The second is measured readability. A `tracing` event's fields are fixed at
//! the macro call, so "omit a field when its value is absent" cannot be written
//! as one call — it is two, selected by a condition. When a dependent crate
//! enables `tracing`'s `log` bridge, Cargo's feature unification turns that on
//! for this crate too, and each macro then expands into the bridge's own
//! branch ladder. Two expansions plus the selecting condition in one function
//! exceed the workspace's cognitive-complexity ceiling without the function
//! having become harder for a reader to follow. Giving every spelling its own
//! small function keeps the complexity of each at the cost of one call's worth,
//! and leaves the entrypoints free to be written as plain dispatch.
//!
//! Every parameter here is a `&'static str` drawn from a closed set declared in
//! `telemetry`, or the already-classified fields of
//! [`CandidateFailureFields`]. Nothing in this module can put a path, a variable
//! value or a file's contents into an event.

use super::telemetry::{CandidateFailureFields, OPERATION_POLICY_RESOLVE};

/// Emit the policy event for a run driven by an explicit selector chain.
pub(super) fn policy_resolution_with_selector(class: &'static str) {
    tracing::debug!(
        event = "discovery.policy",
        operation = OPERATION_POLICY_RESOLVE,
        selector_class = class,
        "configuration file policy resolved its layers"
    );
}

/// Emit the policy event for the automatic branch, where no selector matched.
pub(super) fn policy_resolution_without_selector() {
    tracing::debug!(
        event = "discovery.policy",
        operation = OPERATION_POLICY_RESOLVE,
        "configuration file policy resolved its layers"
    );
}

/// Emit the candidate-failure event for a candidate a scoped walk met.
pub(super) fn candidate_failure_scoped(
    fields: &CandidateFailureFields,
    outcome: &'static str,
    label: &'static str,
) {
    let CandidateFailureFields {
        operation,
        required,
        source,
        category,
        ..
    } = *fields;
    tracing::debug!(
        event = "discovery.candidate",
        operation,
        outcome,
        required,
        source,
        category,
        scope = label,
        "configuration candidate rejected"
    );
}

/// Emit the candidate-failure event for a rung that has no scope.
pub(super) fn candidate_failure_unscoped(fields: &CandidateFailureFields, outcome: &'static str) {
    let CandidateFailureFields {
        operation,
        required,
        source,
        category,
        ..
    } = *fields;
    tracing::debug!(
        event = "discovery.candidate",
        operation,
        outcome,
        required,
        source,
        category,
        "configuration candidate rejected"
    );
}

/// Emit the terminal event for a scoped winner: both optional fields present.
pub(super) fn load_outcome_source_and_scope(
    operation: &'static str,
    outcome: &'static str,
    rung: &'static str,
    label: &'static str,
) {
    tracing::debug!(
        event = "discovery.load",
        operation,
        outcome,
        source = rung,
        scope = label,
        "configuration discovery finished"
    );
}

/// Emit the terminal event for a winner from a rung that carries no scope.
pub(super) fn load_outcome_source(
    operation: &'static str,
    outcome: &'static str,
    rung: &'static str,
) {
    tracing::debug!(
        event = "discovery.load",
        operation,
        outcome,
        source = rung,
        "configuration discovery finished"
    );
}

/// Emit the terminal event for a scoped outcome that has no winning candidate.
pub(super) fn load_outcome_scope(
    operation: &'static str,
    outcome: &'static str,
    label: &'static str,
) {
    tracing::debug!(
        event = "discovery.load",
        operation,
        outcome,
        scope = label,
        "configuration discovery finished"
    );
}

/// Emit the terminal event for an outcome with neither a rung nor a scope.
pub(super) fn load_outcome_bare(operation: &'static str, outcome: &'static str) {
    tracing::debug!(
        event = "discovery.load",
        operation,
        outcome,
        "configuration discovery finished"
    );
}
