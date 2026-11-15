//! Counter contracts for the bounded merge telemetry vocabulary.
//!
//! Split from `merge_telemetry.rs` so each suite stays within the repository's
//! 400-line code file ceiling. The recorder is local to each test thread, so
//! these neither install a process-global recorder nor interfere with another
//! integration target.

#![cfg(feature = "metrics")]

use figment::Provider;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
use ortho_config::{
    CsvEnv, MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource,
    load_and_merge_subcommand_with_sources, subcommand::Prefix,
};
use std::sync::Arc;

#[path = "support/merge_telemetry_fixtures.rs"]
mod merge_telemetry_fixtures;

use merge_telemetry_fixtures::{DerivedConfig, ProfileTelemetryConfig, TelemetrySubcommand};

/// Extract counter values from a debugging-recorder snapshot.
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

/// Read one counter identified by its complete bounded label set.
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

/// Exercise each source-aware merge boundary under a local metrics recorder.
fn recorded_merge_counters() -> Vec<(metrics_util::CompositeKey, u64)> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let csv_source = Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "7"));
        drop(
            CsvEnv::prefixed("MERGE_TELEMETRY_")
                .with_source(csv_source)
                .data(),
        );

        let derived_source = Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "7"));
        let discovery: SharedEnvSource = derived_source.clone();
        let merge: SharedScanEnvSource = derived_source;
        drop(DerivedConfig::load_from_iter_with_sources(
            ["telemetry"],
            discovery,
            merge,
        ));

        drop(ProfileTelemetryConfig::load_with_profile_from_iter([
            "telemetry",
        ]));

        // The ordinary boundary resolves a profile too, so it counts
        // separately from the reporting entry point above.
        drop(ProfileTelemetryConfig::load_from_iter(["telemetry"]));

        let profile_source = Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "7"));
        let profile_discovery: SharedEnvSource = profile_source.clone();
        let profile_merge: SharedScanEnvSource = profile_source;
        drop(
            ProfileTelemetryConfig::load_with_profile_from_iter_with_sources(
                ["telemetry"],
                profile_discovery,
                profile_merge,
            ),
        );

        let subcommand_source = Arc::new(MapEnv::new().with_var("MERGE_CMDS_TELEMETRY_JOBS", "7"));
        drop(load_and_merge_subcommand_with_sources(
            &Prefix::new("MERGE"),
            &TelemetrySubcommand::default(),
            subcommand_source,
        ));

        drop(
            CsvEnv::raw()
                .map(|key| key.into())
                .with_source(Arc::new(MapEnv::new()))
                .data(),
        );
    });

    counters(snapshotter.snapshot())
}

/// Assert each operation emits its expected attempt and success counters
/// under the given source label.
fn assert_operation_counters(
    entries: &[(metrics_util::CompositeKey, u64)],
    source: &'static str,
    operations: &[(&'static str, u64, u64)],
) {
    for (operation, successes, attempts) in operations {
        assert_eq!(
            counter_with_labels(
                entries,
                "ortho_config.merge.attempts",
                &[
                    ("operation", operation),
                    ("source", source),
                    ("outcome", "attempt"),
                    ("category", "none"),
                ],
            ),
            *attempts,
            "expected {attempts} {source} attempt(s) for {operation}: {entries:?}"
        );
        assert_eq!(
            counter_with_labels(
                entries,
                "ortho_config.merge.outcomes",
                &[
                    ("operation", operation),
                    ("source", source),
                    ("outcome", "success"),
                    ("category", "none"),
                ],
            ),
            *successes,
            "expected {successes} {source} success(es) for {operation}: {entries:?}"
        );
    }
}

/// Assert opaque transforms use the closed failure category in metrics labels.
fn assert_opaque_transform_failure(entries: &[(metrics_util::CompositeKey, u64)]) {
    assert_eq!(
        counter_with_labels(
            entries,
            "ortho_config.merge.outcomes",
            &[
                ("operation", "csv_env"),
                ("source", "injected"),
                ("outcome", "failure"),
                ("category", "opaque_key_transform"),
            ],
        ),
        1,
        "expected one bounded opaque-transform failure: {entries:?}"
    );
}

/// Source-aware operations count attempts and bounded terminal outcomes.
#[test]
fn merge_operations_emit_bounded_counters() {
    let entries = recorded_merge_counters();
    // The injected profile load builds its environment provider from the
    // injected merge source, so it adds one successful `csv_env` boundary
    // on top of the three explicit `CsvEnv` calls below.
    assert_operation_counters(
        &entries,
        "injected",
        &[
            ("csv_env", 4, 5),
            ("derived_load", 1, 1),
            ("profile_load", 1, 1),
            ("subcommand_load", 1, 1),
        ],
    );
    // The process-backed profile entry points take no injected source, so
    // their boundary is labelled `process` rather than `injected`. Both
    // the reporting entry point and the ordinary load resolve a profile,
    // so each counts one attempt and one success here.
    assert_operation_counters(&entries, "process", &[("profile_load", 2, 2)]);
    assert_opaque_transform_failure(&entries);
}
