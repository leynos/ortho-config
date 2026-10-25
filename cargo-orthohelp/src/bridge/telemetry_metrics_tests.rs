//! Behavioural contracts for the optional bridge cache metrics.

use super::telemetry_metrics::{CacheOperation, CacheOutcome, record_cache_event};
use super::{BridgeCache, BridgeConfig, load_or_build_ir_inner, write_ir_cache};
use camino::Utf8PathBuf;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
use std::collections::BTreeSet;
use std::error::Error;

const CACHE_EVENT_METRIC: &str = "cargo_orthohelp.bridge_cache.events";

fn bridge_cache(root: &tempfile::TempDir) -> Result<BridgeCache, Box<dyn Error>> {
    let cache_root = Utf8PathBuf::from_path_buf(root.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok(BridgeCache {
        cache_dir: cache_root.clone(),
        target_dir: cache_root.join("target"),
        ir_path: cache_root.join("ir.json"),
    })
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

#[test]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache metric mismatches more clearly than errors"
)]
fn lookup_metrics_cover_hits_misses_and_read_failures() -> Result<(), Box<dyn Error>> {
    let hit_root = tempfile::tempdir()?;
    let hit_cache = bridge_cache(&hit_root)?;
    let hit_config = bridge_config(&hit_root)?;
    std::fs::write(&hit_cache.ir_path, "cached IR")?;

    let miss_root = tempfile::tempdir()?;
    let miss_cache = bridge_cache(&miss_root)?;
    let miss_config = bridge_config(&miss_root)?;

    let failed_root = tempfile::tempdir()?;
    let failed_cache = bridge_cache(&failed_root)?;
    let failed_config = bridge_config(&failed_root)?;
    std::fs::create_dir(&failed_cache.ir_path)?;

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

#[test]
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
    std::fs::create_dir(&failed_read_cache.ir_path)?;

    let failed_write_root = tempfile::tempdir()?;
    let failed_write_cache = bridge_cache(&failed_write_root)?;
    std::fs::remove_dir(&failed_write_cache.cache_dir)?;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        let publish = write_ir_cache(&success_cache, CONTENT);
        assert!(publish.is_ok(), "cache publish failed: {publish:?}");
        let unchanged = write_ir_cache(&success_cache, CONTENT);
        assert!(
            unchanged.is_ok(),
            "identical cache publish failed: {unchanged:?}"
        );
        assert!(write_ir_cache(&failed_read_cache, CONTENT).is_err());
        assert!(write_ir_cache(&failed_write_cache, CONTENT).is_err());
    });

    let counters = metric_counters(snapshotter.snapshot());
    assert_bounded_labels(&counters);
    assert_eq!(event_count(&counters, "publish", "updated"), 1);
    assert_eq!(event_count(&counters, "publish", "unchanged"), 1);
    assert_eq!(event_count(&counters, "publish", "read_failed"), 1);
    assert_eq!(event_count(&counters, "publish", "write_failed"), 1);
    Ok(())
}

#[test]
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
