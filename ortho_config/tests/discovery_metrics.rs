//! Counter emission through the `metrics` facade.
//!
//! Split from `discovery_telemetry.rs` to keep that file within the 400-line
//! ceiling. The recorder is installed per-thread via `with_local_recorder`,
//! not globally: a global recorder can be set only once per process, which
//! would make these tests order-dependent and mutually exclusive — the same
//! trap the injected environment source exists to avoid.

#![cfg(feature = "metrics")]

use ortho_config::MapEnv;

// The builder is shared with `discovery_telemetry.rs` and the injected-source
// suites: the same shape keeps the counter expectations comparable across them.
#[path = "support/discovery_builder.rs"]
mod discovery_builder;

use discovery_builder::discovery_with;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};

/// Count one counter, restricted to a single label.
///
/// Scoped by label rather than totalled: `count_outcome` fires from both
/// `candidate_failure` and `load_outcome`, so a bare total would also be
/// asserting that no candidate failed. That is a host-dependent claim —
/// `/etc/xdg/demo/config.toml` is a real path that could exist on the
/// machine running the test — and it is not what this test is about.
fn counter_for(
    entries: &[(metrics_util::CompositeKey, u64)],
    name: &str,
    label: (&str, &str),
) -> u64 {
    entries
        .iter()
        .filter(|(key, _)| {
            key.key().name() == name
                && key
                    .key()
                    .labels()
                    .any(|found| found.key() == label.0 && found.value() == label.1)
        })
        .map(|(_, count)| *count)
        .sum()
}

fn counters(snapshot: Snapshot) -> Vec<(metrics_util::CompositeKey, u64)> {
    snapshot
        .into_vec()
        .into_iter()
        .filter_map(|(key, _, _, value)| match value {
            DebugValue::Counter(count) => Some((key, count)),
            _ => None,
        })
        .collect()
}

/// Count one counter, requiring every listed label on the same key.
///
/// The single-label helper above is deliberately loose for the shared
/// outcome counter; candidate failures carry three labels that must all
/// belong to one metric key, and accepting any-one-matches would let a
/// mislabelled emission pass.
fn counter_with_labels(
    entries: &[(metrics_util::CompositeKey, u64)],
    name: &str,
    labels: &[(&str, &str)],
) -> u64 {
    entries
        .iter()
        .filter(|(key, _)| {
            key.key().name() == name
                && labels.iter().all(|(label, value)| {
                    key.key()
                        .labels()
                        .any(|found| found.key() == *label && found.value() == *value)
                })
        })
        .map(|(_, count)| *count)
        .sum()
}

/// A failed candidate increments the labelled failure counter exactly once.
#[test]
fn a_candidate_failure_increments_the_labelled_counter() {
    use ortho_config::ConfigDiscovery;
    use std::sync::Arc;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let discovery = ConfigDiscovery::builder("demo")
            .clear_project_roots()
            .add_required_path("/nonexistent/required.toml")
            .env_source(Arc::new(MapEnv::new()))
            .build();
        drop(discovery.load_first_partitioned());
    });

    let entries = counters(snapshotter.snapshot());
    assert_eq!(
        counter_with_labels(
            &entries,
            "ortho_config.discovery.candidate_failures",
            &[
                ("operation", "discover_first"),
                ("source", "required_explicit"),
                ("category", "file"),
            ],
        ),
        1,
        "exactly one fully-labelled candidate failure should be counted, got {entries:?}"
    );
}

#[test]
fn discovery_increments_attempt_and_outcome_counters() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let discovery = discovery_with(MapEnv::new());
        drop(discovery.load_first());
    });

    let entries = counters(snapshotter.snapshot());
    assert_eq!(
        counter_for(
            &entries,
            "ortho_config.discovery.attempts",
            ("operation", "discover_first")
        ),
        1,
        "one discovery attempt should be counted, got {entries:?}"
    );
    assert_eq!(
        counter_for(
            &entries,
            "ortho_config.discovery.outcomes",
            ("outcome", "not_found")
        ),
        1,
        "one terminal not_found outcome should be counted, got {entries:?}"
    );
}
