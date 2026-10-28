//! Read and atomically publish the bridge's cached intermediate representation.
//!
//! The bridge orchestration module is the only production caller. This private
//! module owns bridge-cache file I/O and does not introduce a crate-wide cache
//! or filesystem abstraction.

use super::BridgeCache;
use crate::bridge_observability as observability;
use crate::error::OrthohelpError;
use crate::fs_helpers::open_optional_dir;
use std::io::{Read, Write};
use std::time::Instant;

/// Reads the cached IR JSON, returning `None` when no cache file exists.
pub(super) fn read_cached_ir(paths: &BridgeCache) -> Result<Option<String>, OrthohelpError> {
    let Some(dir) = open_optional_dir(paths.cache_dir.as_path())? else {
        return Ok(None);
    };
    let mut file = match dir.open("ir.json") {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: err,
            });
        }
    };

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .map_err(|err| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: err,
        })?;

    Ok(Some(buffer))
}

/// Writes `json` to the cache, preserving the file when its contents match.
///
/// Idempotent writes preserve modification time across path-normalization
/// cache misses. Read failures are propagated rather than treated as misses,
/// so unreadable cache state cannot be overwritten without inspection.
pub(super) fn write_ir_cache(paths: &BridgeCache, json: &str) -> Result<(), OrthohelpError> {
    let started = Instant::now();
    let existing = read_cached_ir(paths).inspect_err(|_| {
        observability::failed("bridge_ir_cache_publish", "read_failed", started);
        #[cfg(feature = "metrics")]
        super::telemetry_metrics::record_cache_event(
            super::telemetry_metrics::CacheOperation::Publish,
            super::telemetry_metrics::CacheOutcome::ReadFailed,
        );
    })?;
    if existing.as_deref() == Some(json) {
        observability::completed("bridge_ir_cache_publish", "unchanged", started);
        super::telemetry::ir_cache_unchanged(paths);
        #[cfg(feature = "metrics")]
        super::telemetry_metrics::record_cache_event(
            super::telemetry_metrics::CacheOperation::Publish,
            super::telemetry_metrics::CacheOutcome::Unchanged,
        );
        return Ok(());
    }

    let publish = (|| -> Result<(), OrthohelpError> {
        let mut file =
            tempfile::NamedTempFile::new_in(paths.cache_dir.as_std_path()).map_err(|source| {
                OrthohelpError::Io {
                    path: paths.cache_dir.clone(),
                    source,
                }
            })?;
        super::telemetry::ir_cache_write(paths, json.len());
        file.write_all(json.as_bytes())
            .map_err(|source| OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source,
            })?;
        file.persist(paths.ir_path.as_std_path())
            .map_err(|error| OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: error.error,
            })?;
        Ok(())
    })();
    publish.inspect_err(|_| {
        observability::failed("bridge_ir_cache_publish", "write_failed", started);
        #[cfg(feature = "metrics")]
        super::telemetry_metrics::record_cache_event(
            super::telemetry_metrics::CacheOperation::Publish,
            super::telemetry_metrics::CacheOutcome::WriteFailed,
        );
    })?;

    observability::completed("bridge_ir_cache_publish", "updated", started);
    #[cfg(feature = "metrics")]
    super::telemetry_metrics::record_cache_event(
        super::telemetry_metrics::CacheOperation::Publish,
        super::telemetry_metrics::CacheOutcome::Updated,
    );
    Ok(())
}
