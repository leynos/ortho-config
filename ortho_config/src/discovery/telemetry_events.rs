//! One `tracing` callsite per event spelling.
//!
//! Split from `telemetry` for two reasons that reinforce each other.
//!
//! The first is size: the vocabulary, the entrypoints and their documentation
//! together reach the repository's file ceiling, the same pressure that
//! produced `telemetry_counters`.
//!
//! The second is correctness. A `tracing` event's *field names* are fixed by
//! the macro's field list and stored in the callsite's static metadata; only
//! the values are conditional. Recording a `None` omits the value but leaves
//! the name behind, so "omit a field when its value is absent" — which both
//! the [`CandidateFailureFields::scope`] contract and the `discovery.load`
//! contract in `docs/design.md` promise — can only be written as one macro
//! call per field combination. Each combination below is its own function, and
//! each function is one call, so the event a subscriber sees in metadata is
//! exactly the event the documentation describes.
//!
//! Writing those functions out by hand drew a measured `CodeScene` duplicacy
//! finding: the bodies are near-identical, because the difference between them
//! *is* the field list. The `emit_*` macros below expand to a single statement
//! per function, which keeps each body distinct at the level the detector
//! reads while leaving the callsites — and therefore the metadata — separate.
//!
//! Every parameter here is a `&'static str` drawn from a closed set declared in
//! `telemetry`, or the already-classified fields of
//! [`CandidateFailureFields`]. Nothing in this module can put a path, a variable
//! value or a file's contents into an event.

use super::telemetry::{CandidateFailureFields, OPERATION_POLICY_RESOLVE};

/// Expand to the policy event for whichever field subset was supplied.
macro_rules! emit_policy {
    ($operation:expr $(, $field:ident = $value:expr)* $(,)?) => {
        tracing::debug!(
            event = "discovery.policy",
            operation = $operation,
            $($field = $value,)*
            "configuration file policy resolved its layers"
        )
    };
}

/// Expand to the candidate event for whichever field subset was supplied.
macro_rules! emit_candidate {
    ($operation:expr, $outcome:expr, $required:expr, $source:expr, $category:expr
        $(, $field:ident = $value:expr)* $(,)?) => {
        tracing::debug!(
            event = "discovery.candidate",
            operation = $operation,
            outcome = $outcome,
            required = $required,
            source = $source,
            category = $category,
            $($field = $value,)*
            "configuration candidate rejected"
        )
    };
}

/// Expand to the load event for whichever field subset was supplied.
macro_rules! emit_load {
    ($operation:expr, $outcome:expr $(, $field:ident = $value:expr)* $(,)?) => {
        tracing::debug!(
            event = "discovery.load",
            operation = $operation,
            outcome = $outcome,
            $($field = $value,)*
            "configuration discovery finished"
        )
    };
}

/// Emit the policy event for a run driven by an explicit selector chain.
pub(super) fn policy_resolution_with_selector(class: &'static str) {
    emit_policy!(OPERATION_POLICY_RESOLVE, selector_class = class);
}

/// Emit the policy event for the automatic branch, where no selector matched.
pub(super) fn policy_resolution_without_selector() {
    emit_policy!(OPERATION_POLICY_RESOLVE);
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
    emit_candidate!(
        operation,
        outcome,
        required,
        source,
        category,
        scope = label
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
    emit_candidate!(operation, outcome, required, source, category);
}

/// Emit the terminal event for a scoped winner: both optional fields present.
pub(super) fn load_outcome_source_and_scope(
    operation: &'static str,
    outcome: &'static str,
    source: &'static str,
    scope: &'static str,
) {
    emit_load!(operation, outcome, source = source, scope = scope);
}

/// Emit the terminal event for a winner from a rung that carries no scope.
pub(super) fn load_outcome_source(
    operation: &'static str,
    outcome: &'static str,
    source: &'static str,
) {
    emit_load!(operation, outcome, source = source);
}

/// Emit the terminal event for a scoped outcome that has no winning candidate.
pub(super) fn load_outcome_scope(
    operation: &'static str,
    outcome: &'static str,
    scope: &'static str,
) {
    emit_load!(operation, outcome, scope = scope);
}

/// Emit the terminal event for an outcome with neither a rung nor a scope.
pub(super) fn load_outcome_bare(operation: &'static str, outcome: &'static str) {
    emit_load!(operation, outcome);
}
