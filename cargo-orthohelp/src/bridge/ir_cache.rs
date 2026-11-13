//! Read and atomically publish the bridge's cached intermediate representation.
//!
//! The bridge orchestration module is the only production caller. This private
//! module owns bridge-cache file I/O and does not introduce a crate-wide cache
//! or filesystem abstraction. It reports what happened through
//! [`PublishOutcome`] and [`PublishError`] rather than recording tracing or
//! metrics itself, so observability policy stays at the orchestration boundary.

use super::BridgeCache;
use crate::error::OrthohelpError;
use crate::fs_helpers::open_optional_dir;
use std::io::{Read, Write};

/// What publishing the generated IR to the cache did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PublishOutcome {
    /// The cache already held identical IR, so the file was left untouched.
    Unchanged,
    /// Newly generated IR replaced the cache entry.
    Updated,
}

/// The cache I/O stage that produced a [`PublishError`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PublishStage {
    /// Reading the existing cache entry failed.
    Read,
    /// Creating, writing, or replacing the cache entry failed.
    Write,
}

/// A publish failure, tagged with the stage that produced it.
///
/// The caller needs the stage to name the outcome in tracing and metrics; the
/// underlying [`OrthohelpError`] alone does not distinguish a read from a
/// write that failed for the same path.
#[derive(Debug)]
pub(super) struct PublishError {
    /// Stage whose I/O operation failed.
    stage: PublishStage,
    /// Underlying cache I/O failure, preserved for the caller to report.
    source: OrthohelpError,
}

impl PublishError {
    /// Return the stage at which publishing failed.
    pub(super) const fn stage(&self) -> PublishStage {
        self.stage
    }

    /// Return the underlying cache I/O failure.
    pub(super) fn into_source(self) -> OrthohelpError {
        self.source
    }
}

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

/// Reports whether the cached IR is byte-for-byte identical to `json`.
///
/// The comparison is bounded by `json`'s length: the cache file's size is
/// checked first, and the body is then read through a `take` limit of
/// `json.len() + 1` bytes. A cache entry whose length differs can therefore
/// never be read into memory, which keeps an attacker-influenced `ir.json`
/// from forcing an allocation proportional to its own size on the normal
/// publish path.
///
/// The limit is one byte beyond the expected payload so that a file which
/// grows between the metadata check and the read still fails the comparison
/// instead of being silently truncated into a false match.
///
/// Read failures are propagated rather than treated as misses, so unreadable
/// cache state cannot be overwritten without inspection. An entry that is not
/// a regular file is rejected for the same reason: a directory has no body to
/// compare, and reading any other special file could block instead of
/// returning, so neither can be reported as a harmless content mismatch.
fn cached_ir_matches(paths: &BridgeCache, json: &str) -> Result<bool, OrthohelpError> {
    let Some(dir) = open_optional_dir(paths.cache_dir.as_path())? else {
        return Ok(false);
    };
    let file = match dir.open("ir.json") {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: err,
            });
        }
    };

    let expected_len = json.len() as u64;
    let metadata = file.metadata().map_err(|err| OrthohelpError::Io {
        path: paths.ir_path.clone(),
        source: err,
    })?;
    if !metadata.is_file() {
        return Err(OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cache entry is not a regular file",
            ),
        });
    }
    if metadata.len() != expected_len {
        return Ok(false);
    }

    let mut buffer = Vec::with_capacity(json.len().saturating_add(1));
    file.take(expected_len.saturating_add(1))
        .read_to_end(&mut buffer)
        .map_err(|err| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: err,
        })?;

    Ok(buffer == json.as_bytes())
}

/// Writes `json` to the cache, preserving the file when its contents match.
///
/// Idempotent writes preserve modification time across path-normalization
/// cache misses. The existing entry is compared only after a metadata length
/// check and a read capped at the new payload's length plus one byte, so a
/// large mismatched cache file is never loaded. Read failures are propagated
/// rather than treated as misses, so unreadable cache state cannot be
/// overwritten without inspection.
pub(super) fn write_ir_cache(
    paths: &BridgeCache,
    json: &str,
) -> Result<PublishOutcome, PublishError> {
    let matches = cached_ir_matches(paths, json).map_err(|source| PublishError {
        stage: PublishStage::Read,
        source,
    })?;
    if matches {
        return Ok(PublishOutcome::Unchanged);
    }

    let mut file =
        tempfile::NamedTempFile::new_in(paths.cache_dir.as_std_path()).map_err(|source| {
            PublishError {
                stage: PublishStage::Write,
                source: OrthohelpError::Io {
                    path: paths.cache_dir.clone(),
                    source,
                },
            }
        })?;
    file.write_all(json.as_bytes())
        .map_err(|source| PublishError {
            stage: PublishStage::Write,
            source: OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source,
            },
        })?;
    file.persist(paths.ir_path.as_std_path())
        .map_err(|error| PublishError {
            stage: PublishStage::Write,
            source: OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: error.error,
            },
        })?;

    Ok(PublishOutcome::Updated)
}
