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

/// Opens the cached IR without blocking on a special file.
///
/// A read-only open of a FIFO waits for a writer, so an `ir.json` replaced by
/// one would stall the bridge build before any type check could run. On Unix
/// the open therefore carries `O_NONBLOCK`, which returns immediately; the
/// caller then rejects the opened handle because it is not a regular file.
/// The check inspects the handle rather than the path, so an entry that changes
/// between the open and the check cannot substitute a special file for a
/// regular one.
#[cfg(unix)]
fn open_cache_entry(dir: &cap_std::fs_utf8::Dir) -> std::io::Result<CacheEntry> {
    use cap_std::fs_utf8::OpenOptionsExt;

    let nonblock = i32::try_from(rustix::fs::OFlags::NONBLOCK.bits())
        .map_err(|_| std::io::Error::other("O_NONBLOCK does not fit in the open flags word"))?;
    let mut options = cap_std::fs_utf8::OpenOptions::new();
    options.read(true).custom_flags(nonblock);

    dir.open_with("ir.json", &options)
        .map(|file| CacheEntry { file })
}

/// Opens the cached IR without blocking on a special file.
///
/// Windows places named pipes in their own namespace, which a
/// directory-relative open cannot reach, so the hazard that motivates the
/// non-blocking Unix open has no equivalent here.
#[cfg(not(unix))]
fn open_cache_entry(dir: &cap_std::fs_utf8::Dir) -> std::io::Result<CacheEntry> {
    dir.open("ir.json").map(|file| CacheEntry { file })
}

/// The opened cache entry, wrapped so every body read is observable.
///
/// The inner handle is private, so code between the open and the comparison
/// can only reach cached content through the [`Read`] implementation here.
/// The publish-path test counts those reads; without the wrapper, a body read
/// added to [`cached_ir_matches`] outside the comparison would be invisible to
/// every test.
struct CacheEntry {
    /// The opened handle, reachable for metadata and counted reads only.
    file: cap_std::fs_utf8::File,
}

impl CacheEntry {
    /// Returns the metadata of the opened entry.
    fn metadata(&self) -> std::io::Result<cap_std::fs_utf8::Metadata> {
        self.file.metadata()
    }
}

impl Read for CacheEntry {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.file.read(buffer)?;
        #[cfg(test)]
        read_observation::record(read);
        Ok(read)
    }
}

/// Reports whether the cached IR is byte-for-byte identical to `json`.
///
/// The entry is opened, its metadata is read, and the whole comparison is
/// delegated to [`compare_cache_entry`]. This function never touches the body
/// itself, and the handle it passes on is a [`CacheEntry`], so a body read
/// added here would still be counted by the publish-path test instead of
/// escaping it.
///
/// Read failures are propagated rather than treated as misses, so unreadable
/// cache state cannot be overwritten without inspection.
fn cached_ir_matches(paths: &BridgeCache, json: &str) -> Result<bool, OrthohelpError> {
    let Some(dir) = open_optional_dir(paths.cache_dir.as_path())? else {
        return Ok(false);
    };
    let file = match open_cache_entry(&dir) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: err,
            });
        }
    };

    let metadata = file.metadata().map_err(|err| OrthohelpError::Io {
        path: paths.ir_path.clone(),
        source: err,
    })?;

    compare_cache_entry(file, metadata.len(), metadata.is_file(), json).map_err(|source| {
        OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source,
        }
    })
}

/// Compares an opened cache entry with `json`, reading at most `json.len() + 1`
/// bytes.
///
/// This is the whole comparison a publish performs: [`cached_ir_matches`]
/// opens the entry and hands the handle here, and nothing else on the publish
/// path reads cached content. Keeping the regular-file rejection, the length
/// check and the bounded read in one function lets the bound be tested through
/// a reader that records consumption, rather than through allocation
/// instrumentation.
///
/// `cached_len` is the size metadata reported. When it disagrees with `json`
/// the body is not read at all, so a file-sized mismatch costs one `stat` and
/// no allocation. The limit is one byte beyond the expected payload so that a
/// file which grows between the metadata check and the read still fails the
/// comparison instead of being silently truncated into a false match.
///
/// An entry that is not a regular file is rejected rather than compared: a
/// directory has no body to compare, and reading or blocking in the open of
/// any other special file would stall the build, so neither can be reported as
/// a harmless content mismatch.
fn compare_cache_entry(
    body: impl Read,
    cached_len: u64,
    is_regular_file: bool,
    json: &str,
) -> std::io::Result<bool> {
    if !is_regular_file {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "cache entry is not a regular file",
        ));
    }

    let expected_len = json.len() as u64;
    if cached_len != expected_len {
        return Ok(false);
    }

    let mut buffer = Vec::with_capacity(json.len().saturating_add(1));
    body.take(expected_len.saturating_add(1))
        .read_to_end(&mut buffer)?;

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

#[cfg(test)]
#[path = "ir_cache_tests.rs"]
mod tests;

// The bounded-comparison tests live in their own modules so that no file
// outgrows the repository's per-file length limit.
#[cfg(test)]
#[path = "ir_cache_bounded_tests.rs"]
mod bounded_tests;

#[cfg(test)]
#[path = "ir_cache_bounded_proptests.rs"]
mod bounded_proptests;

// Counts body reads on the publish path and publishes through
// `write_ir_cache`, so a read added outside the comparison is still observed.
#[cfg(test)]
#[path = "ir_cache_read_observation.rs"]
mod read_observation;

#[cfg(test)]
#[path = "ir_cache_publish_tests.rs"]
mod publish_tests;
