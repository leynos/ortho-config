//! Behavioural contracts for the optional bridge cache metrics.

use super::ir_cache::{PublishError, PublishOutcome, write_ir_cache};
use super::telemetry_metrics::{CacheOperation, CacheOutcome, record_cache_event};
use super::{BridgeCache, BridgeConfig, load_or_build_ir_inner, publish_ir_cache};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
use rstest::rstest;
use std::collections::BTreeSet;
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};

const CACHE_EVENT_METRIC: &str = "cargo_orthohelp.bridge_cache.events";

fn bridge_cache(root: &tempfile::TempDir) -> Result<BridgeCache, Box<dyn Error>> {
    let cache_root = Utf8PathBuf::from_path_buf(root.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok(bridge_cache_at(&cache_root))
}

fn bridge_cache_at(cache_root: &Utf8PathBuf) -> BridgeCache {
    BridgeCache {
        cache_dir: cache_root.clone(),
        target_dir: cache_root.join("target"),
        ir_path: cache_root.join("ir.json"),
    }
}

fn bridge_config(root: &tempfile::TempDir) -> Result<BridgeConfig, Box<dyn Error>> {
    let package_root = Utf8PathBuf::from_path_buf(root.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok(BridgeConfig {
        workspace_root: package_root.clone(),
        package_root,
        package_name: "fixture".to_owned(),
        root_type: "fixture::Config".to_owned(),
        ortho_config_dependency: super::OrthoConfigDependency {
            requirement: "0.10.0".to_owned(),
            path: None,
        },
    })
}

fn metric_counters(snapshot: Snapshot) -> Vec<(metrics_util::CompositeKey, u64)> {
    snapshot
        .into_vec()
        .into_iter()
        .filter_map(|(key, _, _, value)| match value {
            DebugValue::Counter(count) => Some((key, count)),
            _ => None,
        })
        .collect()
}

fn event_count(
    counters: &[(metrics_util::CompositeKey, u64)],
    operation: &str,
    outcome: &str,
) -> u64 {
    counters
        .iter()
        .filter(|(key, _)| {
            let labels = key.key().labels().collect::<Vec<_>>();
            key.key().name() == CACHE_EVENT_METRIC
                && labels.len() == 2
                && labels
                    .iter()
                    .any(|label| label.key() == "operation" && label.value() == operation)
                && labels
                    .iter()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
        })
        .map(|(_, count)| *count)
        .sum()
}

fn assert_bounded_labels(counters: &[(metrics_util::CompositeKey, u64)]) {
    let operations = BTreeSet::from(["lookup", "publish"]);
    let outcomes = BTreeSet::from([
        "hit",
        "miss",
        "read_failed",
        "unchanged",
        "updated",
        "write_failed",
    ]);

    for (key, _) in counters {
        let labels = key.key().labels().collect::<Vec<_>>();
        assert_eq!(key.key().name(), CACHE_EVENT_METRIC);
        assert_eq!(labels.len(), 2, "metric labels must stay limited: {key:?}");
        assert!(
            labels
                .iter()
                .any(|label| { label.key() == "operation" && operations.contains(label.value()) })
        );
        assert!(
            labels
                .iter()
                .any(|label| label.key() == "outcome" && outcomes.contains(label.value()))
        );
    }
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache metric mismatches more clearly than errors"
)]
fn lookup_metrics_cover_hits_misses_and_read_failures() -> Result<(), Box<dyn Error>> {
    let hit_root = tempfile::tempdir()?;
    let hit_cache = bridge_cache(&hit_root)?;
    let hit_config = bridge_config(&hit_root)?;
    let hit_directory = Dir::open_ambient_dir(&hit_cache.cache_dir, ambient_authority())?;
    hit_directory.write("ir.json", b"cached IR")?;

    let miss_root = tempfile::tempdir()?;
    let miss_cache = bridge_cache(&miss_root)?;
    let miss_config = bridge_config(&miss_root)?;

    let failed_root = tempfile::tempdir()?;
    let failed_cache = bridge_cache(&failed_root)?;
    let failed_config = bridge_config(&failed_root)?;
    let failed_directory = Dir::open_ambient_dir(&failed_cache.cache_dir, ambient_authority())?;
    failed_directory.create_dir("ir.json")?;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        assert!(
            matches!(
                load_or_build_ir_inner(&hit_config, &hit_cache, true, true).as_deref(),
                Ok("cached IR")
            ),
            "lookup should return the cached IR"
        );
        assert!(load_or_build_ir_inner(&miss_config, &miss_cache, true, true).is_err());
        assert!(load_or_build_ir_inner(&failed_config, &failed_cache, true, true).is_err());
    });

    let counters = metric_counters(snapshotter.snapshot());
    assert_bounded_labels(&counters);
    assert_eq!(event_count(&counters, "lookup", "hit"), 1);
    assert_eq!(event_count(&counters, "lookup", "miss"), 1);
    assert_eq!(event_count(&counters, "lookup", "read_failed"), 1);
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache metric mismatches more clearly than errors"
)]
fn publish_metrics_cover_update_unchanged_and_failures() -> Result<(), Box<dyn Error>> {
    const CONTENT: &str = "cached IR";

    let success_root = tempfile::tempdir()?;
    let success_cache = bridge_cache(&success_root)?;

    let failed_read_root = tempfile::tempdir()?;
    let failed_read_cache = bridge_cache(&failed_read_root)?;
    let failed_read_directory =
        Dir::open_ambient_dir(&failed_read_cache.cache_dir, ambient_authority())?;
    failed_read_directory.create_dir("ir.json")?;

    let failed_write_root = tempfile::tempdir()?;
    let failed_write_root_path = Utf8PathBuf::from_path_buf(failed_write_root.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    let missing_cache_root = failed_write_root_path.join("missing-cache");
    let failed_write_cache = bridge_cache_at(&missing_cache_root);

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        // Metrics are recorded by the orchestration boundary, not by the cache
        // unit, so the events must be driven through `publish_ir_cache`.
        let publish = publish_ir_cache(&success_cache, CONTENT);
        assert!(publish.is_ok(), "cache publish failed: {publish:?}");
        let unchanged = publish_ir_cache(&success_cache, CONTENT);
        assert!(
            unchanged.is_ok(),
            "identical cache publish failed: {unchanged:?}"
        );
        assert!(publish_ir_cache(&failed_read_cache, CONTENT).is_err());
        assert!(publish_ir_cache(&failed_write_cache, CONTENT).is_err());
    });

    let counters = metric_counters(snapshotter.snapshot());
    assert_bounded_labels(&counters);
    assert_eq!(event_count(&counters, "publish", "updated"), 1);
    assert_eq!(event_count(&counters, "publish", "unchanged"), 1);
    assert_eq!(event_count(&counters, "publish", "read_failed"), 1);
    assert_eq!(event_count(&counters, "publish", "write_failed"), 1);
    Ok(())
}

/// The cache unit reports what it did and records nothing.
///
/// Observation belongs to the bridge workflow: keeping `write_ir_cache` free
/// of tracing and metrics lets it be tested for persistence alone, and stops a
/// second caller from double-counting an event the boundary already recorded.
#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache outcome mismatches more clearly than errors"
)]
fn write_ir_cache_reports_outcomes_without_recording_metrics() -> Result<(), Box<dyn Error>> {
    const CONTENT: &str = "cached IR";
    const OTHER: &str = "refreshed IR";

    let root = tempfile::tempdir()?;
    let cache = bridge_cache(&root)?;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || -> Result<(), Box<dyn Error>> {
        assert_eq!(
            write_ir_cache(&cache, CONTENT).map_err(PublishError::into_source)?,
            PublishOutcome::Updated,
            "a first publish writes the cache"
        );
        assert_eq!(
            write_ir_cache(&cache, CONTENT).map_err(PublishError::into_source)?,
            PublishOutcome::Unchanged,
            "identical content leaves the cache entry untouched"
        );
        assert_eq!(
            write_ir_cache(&cache, OTHER).map_err(PublishError::into_source)?,
            PublishOutcome::Updated,
            "changed content replaces the cache entry"
        );
        Ok(())
    })?;

    assert!(
        metric_counters(snapshotter.snapshot()).is_empty(),
        "the cache unit must not record metrics; the orchestration boundary does"
    );
    Ok(())
}

/// The CLI's installed recorder is the production collection path.
///
/// `main` installs this recorder before any bridge work, so the counters the
/// cache boundary emits reach the operator's `tracing` pipeline instead of the
/// `metrics` facade's discarding default. The test drives one real emission
/// rather than calling the recorder directly, so it fails if the event shape,
/// the `tracing` target, or the install hook regresses.
#[rstest]
fn installed_recorder_exports_cache_events_to_tracing() {
    crate::metrics_recorder::install();

    let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer({
            let writer_buffer = Arc::clone(&captured);
            move || CapturedWriter(Arc::clone(&writer_buffer))
        })
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        record_cache_event(CacheOperation::Lookup, CacheOutcome::Hit);
    });

    let output = String::from_utf8(
        captured
            .lock()
            .expect("captured tracing output is readable")
            .clone(),
    )
    .expect("tracing output is UTF-8");

    assert!(
        output.contains(CACHE_EVENT_METRIC),
        "the exported event must name the metric: {output}"
    );
    assert!(
        output.contains("operation=lookup") && output.contains("outcome=hit"),
        "the exported event must carry the bounded labels: {output}"
    );
}

/// Writer that collects a subscriber's rendered output for assertions.
struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        // A poisoned lock means another test thread panicked mid-write; report
        // it as a write failure rather than panicking the subscriber.
        let mut captured = self
            .0
            .lock()
            .map_err(|_| io::Error::other("captured tracing output is poisoned"))?;
        captured.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[rstest]
fn metric_labels_are_drawn_from_closed_vocabularies() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        for (operation, outcome) in [
            (CacheOperation::Lookup, CacheOutcome::Hit),
            (CacheOperation::Lookup, CacheOutcome::Miss),
            (CacheOperation::Lookup, CacheOutcome::ReadFailed),
            (CacheOperation::Publish, CacheOutcome::Unchanged),
            (CacheOperation::Publish, CacheOutcome::Updated),
            (CacheOperation::Publish, CacheOutcome::ReadFailed),
            (CacheOperation::Publish, CacheOutcome::WriteFailed),
        ] {
            record_cache_event(operation, outcome);
        }
    });

    let counters = metric_counters(snapshotter.snapshot());
    assert_eq!(counters.len(), 7);
    assert_bounded_labels(&counters);
}
