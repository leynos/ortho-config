//! Bounded-vocabulary assertions shared by the merge telemetry suites.
//!
//! Split from `merge_telemetry.rs` so each suite stays within the repository's
//! 400-line code file ceiling, and so the redaction check has one definition:
//! two copies would drift in which fields count as allowed.

use super::capture_support::Captured;

/// Field names a bounded merge event may carry.
const ALLOWED_FIELDS: &[&str] = &[
    "event",
    "message",
    "operation",
    "source",
    "outcome",
    "category",
];

/// Input values that must never appear in an emitted field.
const SENSITIVE_INPUTS: &[&str] = &[
    "MERGE_TELEMETRY_JOBS",
    "MERGE_CMDS_TELEMETRY_JOBS",
    "UNRELATED_SECRET_KEY",
    "secret-injected-value",
    "/secret/injected/path",
];

fn merge_events(events: &[Captured]) -> impl Iterator<Item = &Captured> {
    events
        .iter()
        .filter(|event| event.field("event") == "merge.layer")
}

/// Find the single merge event matching an operation, source, and outcome.
///
/// # Panics
///
/// Panics when the capture does not hold exactly one such event, so a case
/// that silently recorded nothing fails rather than asserting against an
/// absent event.
pub fn find_event<'events>(
    events: &'events [Captured],
    operation: &str,
    source: &str,
    outcome: &str,
) -> &'events Captured {
    let matching_events = merge_events(events)
        .filter(|event| {
            event.field("operation") == operation
                && event.field("source") == source
                && event.field("outcome") == outcome
        })
        .collect::<Vec<_>>();
    match matching_events.as_slice() {
        [event] => event,
        _ => panic!("captured events must contain exactly one bounded merge event"),
    }
}

/// Assert every merge event uses only allowed fields and leaks no input.
///
/// # Panics
///
/// Panics when the capture holds no merge event at all, or when an event
/// carries an unexpected field name or echoes a sensitive input value.
pub fn assert_bounded_and_redacted(events: &[Captured]) {
    let merge_events = merge_events(events).collect::<Vec<_>>();
    assert!(
        !merge_events.is_empty(),
        "redaction checks require captured merge events"
    );

    for event in merge_events {
        for (name, value) in event.fields() {
            assert!(
                ALLOWED_FIELDS.contains(&name),
                "unexpected merge event field `{name}`"
            );
            for sensitive in SENSITIVE_INPUTS {
                assert!(
                    !value.contains(sensitive),
                    "field `{name}` leaked `{sensitive}`: {value}"
                );
            }
        }
    }
}
