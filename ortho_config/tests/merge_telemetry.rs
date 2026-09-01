//! Telemetry contracts for process-backed and injected environment merging.
//!
//! The merge layer handles environment data that must never reach an event.
//! These cases pin its small, closed telemetry vocabulary and exercise every
//! source-aware entry point that makes a terminal loading decision.

use clap::Parser;
use figment::{Jail, Provider};
use ortho_config::{
    CsvEnv, MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource,
    load_and_merge_subcommand_with_sources, subcommand::Prefix,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[path = "support/tracing_capture.rs"]
#[expect(
    dead_code,
    reason = "This standalone suite needs only capture; other shared helpers serve discovery suites."
)]
mod capture_support;

use capture_support::{Captured, capture};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MERGE_TELEMETRY_")]
struct DerivedConfig {
    jobs: u16,
}

#[derive(Debug, Default, Deserialize, Parser, Serialize)]
#[command(name = "telemetry")]
struct TelemetrySubcommand {
    #[arg(long)]
    jobs: Option<u16>,
}

const ALLOWED_FIELDS: &[&str] = &[
    "event",
    "message",
    "operation",
    "source",
    "outcome",
    "category",
];
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

fn find_event<'events>(
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

fn assert_bounded_and_redacted(events: &[Captured]) {
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

#[test]
fn csv_env_reports_process_and_injected_success_without_input_data() {
    Jail::expect_with(|jail| -> Result<(), figment::Error> {
        jail.clear_env();
        jail.set_env("MERGE_TELEMETRY_JOBS", "7");
        let process_events = capture(|| {
            let result = CsvEnv::prefixed("MERGE_TELEMETRY_").data();
            assert!(result.is_ok(), "process provider should load: {result:?}");
        });
        let process_success = find_event(&process_events, "csv_env", "process", "success");
        assert_eq!(process_success.field("category"), "none");

        jail.clear_env();
        let injected = Arc::new(
            MapEnv::new()
                .with_var("MERGE_TELEMETRY_JOBS", "7")
                .with_var("UNRELATED_SECRET_KEY", "secret-injected-value")
                .with_var("UNRELATED_PATH", "/secret/injected/path"),
        );
        let injected_events = capture(|| {
            let result = CsvEnv::prefixed("MERGE_TELEMETRY_")
                .with_source(injected)
                .data();
            assert!(result.is_ok(), "injected provider should load: {result:?}");
        });
        let injected_success = find_event(&injected_events, "csv_env", "injected", "success");
        assert_eq!(injected_success.field("category"), "none");

        assert_bounded_and_redacted(&process_events);
        assert_bounded_and_redacted(&injected_events);
        Ok(())
    });
}

#[test]
fn opaque_injected_transform_emits_a_bounded_failure() {
    let events = capture(|| {
        let result = CsvEnv::raw()
            .map(|key| key.into())
            .with_source(Arc::new(
                MapEnv::new().with_var("UNRELATED_SECRET_KEY", "secret-injected-value"),
            ))
            .data();
        assert!(result.is_err(), "opaque injected transform must fail");
    });

    let failure = find_event(&events, "csv_env", "injected", "failure");
    assert_eq!(failure.field("category"), "opaque_key_transform");
    assert_bounded_and_redacted(&events);
}

#[test]
fn source_aware_derived_load_reports_success_and_failure() {
    let source = Arc::new(
        MapEnv::new()
            .with_var("MERGE_TELEMETRY_JOBS", "7")
            .with_var("UNRELATED_SECRET_KEY", "secret-injected-value")
            .with_var("UNRELATED_PATH", "/secret/injected/path"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let success_events = capture(|| {
        let result = DerivedConfig::load_from_iter_with_sources(["telemetry"], discovery, merge);
        assert!(
            result.is_ok(),
            "derived injected load should succeed: {result:?}"
        );
    });
    let success = find_event(&success_events, "derived_load", "injected", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    let failing_source =
        Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "secret-injected-value"));
    let failing_discovery: SharedEnvSource = failing_source.clone();
    let failing_merge: SharedScanEnvSource = failing_source;
    let failure_events = capture(|| {
        let result = DerivedConfig::load_from_iter_with_sources(
            ["telemetry"],
            failing_discovery,
            failing_merge,
        );
        assert!(
            result.is_err(),
            "invalid injected value must fail derived loading"
        );
    });
    let failure = find_event(&failure_events, "derived_load", "injected", "failure");
    assert_eq!(failure.field("category"), "merge");
    assert_bounded_and_redacted(&failure_events);
}

#[test]
fn source_aware_subcommand_load_reports_success_and_failure() {
    let success_events = capture(|| {
        let source = Arc::new(MapEnv::new().with_var("MERGE_CMDS_TELEMETRY_JOBS", "7"));
        let result = load_and_merge_subcommand_with_sources(
            &Prefix::new("MERGE"),
            &TelemetrySubcommand::default(),
            source,
        );
        assert!(
            result.is_ok(),
            "subcommand injected load should succeed: {result:?}"
        );
    });
    let success = find_event(&success_events, "subcommand_load", "injected", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    let failure_events = capture(|| {
        let source =
            Arc::new(MapEnv::new().with_var("MERGE_CMDS_TELEMETRY_JOBS", "secret-injected-value"));
        let result = load_and_merge_subcommand_with_sources(
            &Prefix::new("MERGE"),
            &TelemetrySubcommand::default(),
            source,
        );
        assert!(
            result.is_err(),
            "invalid injected value must fail subcommand loading"
        );
    });
    let failure = find_event(&failure_events, "subcommand_load", "injected", "failure");
    assert_eq!(failure.field("category"), "merge");
    assert_bounded_and_redacted(&failure_events);
}

#[cfg(feature = "metrics")]
mod metrics_tests {
    //! Counter contracts for the bounded merge telemetry vocabulary.
    //!
    //! The recorder is local to this thread so the tests neither install a
    //! process-global recorder nor interfere with another integration target.

    use super::*;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};

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

            let subcommand_source =
                Arc::new(MapEnv::new().with_var("MERGE_CMDS_TELEMETRY_JOBS", "7"));
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

    /// Assert each operation emits its expected injected attempt and success counters.
    fn assert_operation_counters(entries: &[(metrics_util::CompositeKey, u64)]) {
        for (operation, successes, attempts) in [
            ("csv_env", 3, 4),
            ("derived_load", 1, 1),
            ("subcommand_load", 1, 1),
        ] {
            assert_eq!(
                counter_with_labels(
                    entries,
                    "ortho_config.merge.attempts",
                    &[
                        ("operation", operation),
                        ("source", "injected"),
                        ("outcome", "attempt"),
                        ("category", "none"),
                    ],
                ),
                attempts,
                "expected {attempts} injected attempt(s) for {operation}: {entries:?}"
            );
            assert_eq!(
                counter_with_labels(
                    entries,
                    "ortho_config.merge.outcomes",
                    &[
                        ("operation", operation),
                        ("source", "injected"),
                        ("outcome", "success"),
                        ("category", "none"),
                    ],
                ),
                successes,
                "expected {successes} injected success(es) for {operation}: {entries:?}"
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
        assert_operation_counters(&entries);
        assert_opaque_transform_failure(&entries);
    }
}
