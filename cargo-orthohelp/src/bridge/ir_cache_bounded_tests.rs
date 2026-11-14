//! Tests for the bounded IR cache comparison and idempotent publishing.
//!
//! The read bound is proved through [`bounded_body_matches`] and a reader that
//! records how many bytes it surrendered, so the tests do not depend on
//! allocation instrumentation. Filesystem behaviour is exercised through a
//! capability directory owned by a shared `rstest` fixture.

use std::io::{self, Read};
use std::time::{Duration, Instant, SystemTime};

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};
use tempfile::tempdir;

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A temporary cache directory plus the bridge paths that point into it.
///
/// The `TempDir` guard is retained so the directory outlives the fixture, and
/// the capability directory is the only handle tests use to touch it.
struct CacheFixture {
    _temporary: tempfile::TempDir,
    dir: Dir,
    paths: BridgeCache,
}

#[fixture]
fn cache_fixture() -> Result<CacheFixture, Box<dyn std::error::Error>> {
    let temporary = tempdir()?;
    let cache_dir = Utf8PathBuf::from_path_buf(temporary.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    let dir = Dir::open_ambient_dir(&cache_dir, ambient_authority())?;
    let paths = BridgeCache {
        cache_dir: cache_dir.clone(),
        target_dir: cache_dir.join("target"),
        ir_path: cache_dir.join("ir.json"),
    };

    Ok(CacheFixture {
        _temporary: temporary,
        dir,
        paths,
    })
}

fn read_mtime(dir: &Dir, relative_path: &str) -> std::io::Result<SystemTime> {
    dir.metadata(relative_path)
        .and_then(|metadata| metadata.modified())
        .map(cap_std::time::SystemTime::into_std)
}

fn poll_mtime_until(
    dir: &Dir,
    relative_path: &str,
    timeout: Duration,
    matches: impl Fn(SystemTime) -> bool,
) -> std::io::Result<SystemTime> {
    let deadline = Instant::now() + timeout;
    let mut mtime = read_mtime(dir, relative_path)?;
    while !matches(mtime) {
        if Instant::now() >= deadline {
            return Err(std::io::Error::other(format!(
                "cache file mtime did not reach the expected value before timeout: {mtime:?}"
            )));
        }
        std::thread::sleep(Duration::from_millis(5));
        mtime = read_mtime(dir, relative_path)?;
    }
    Ok(mtime)
}

fn poll_clock_after(reference: SystemTime, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while SystemTime::now() <= reference {
        assert!(
            Instant::now() < deadline,
            "system clock did not advance beyond {reference:?} before timeout"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A body reader that records how many bytes the comparison consumes.
struct CountingReader<'a> {
    body: &'a [u8],
    consumed: usize,
    read_calls: usize,
}

impl Read for CountingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_calls += 1;
        let start = self.consumed;
        let end = start.saturating_add(buffer.len()).min(self.body.len());
        let available = self
            .body
            .get(start..end)
            .ok_or_else(|| io::Error::other("reader position falls outside the body"))?;
        let target = buffer
            .get_mut(..available.len())
            .ok_or_else(|| io::Error::other("read buffer is shorter than the body slice"))?;
        target.copy_from_slice(available);
        self.consumed = end;
        Ok(available.len())
    }
}

impl<'a> CountingReader<'a> {
    fn new(body: &'a [u8]) -> Self {
        CountingReader {
            body,
            consumed: 0,
            read_calls: 0,
        }
    }
}

#[test]
fn a_length_mismatch_performs_no_body_reads() {
    let json = "cached IR";
    let mut reader = CountingReader::new(b"a cached body that is plainly longer");
    let cached_len = reader.body.len() as u64;

    let matches = bounded_body_matches(&mut reader, cached_len, json).expect("compare body");

    assert!(!matches, "a longer body must not match");
    assert_eq!(
        reader.read_calls, 0,
        "a metadata length mismatch must not read the body at all"
    );
}

#[test]
fn an_equal_length_comparison_reads_at_most_one_byte_past_the_payload() {
    let json = "cached IR";
    let grown = vec![b'x'; json.len() + 64];
    let mut reader = CountingReader::new(&grown);

    let matches = bounded_body_matches(&mut reader, json.len() as u64, json).expect("compare body");

    assert!(!matches, "a body that grew must not masquerade as a match");
    assert_eq!(
        reader.consumed,
        json.len() + 1,
        "the read must stop at one byte past the new payload"
    );
}

#[test]
fn an_identical_body_matches_without_overreading() {
    let json = "cached IR";
    let mut reader = CountingReader::new(json.as_bytes());

    let matches = bounded_body_matches(&mut reader, json.len() as u64, json).expect("compare body");

    assert!(matches, "identical content must match");
    assert_eq!(
        reader.consumed,
        json.len(),
        "an exact match must consume exactly the payload"
    );
}

/// An unreadable cache entry fails the publish instead of being overwritten.
///
/// The cache path is a directory rather than a regular file. Opening it may
/// succeed without yielding a readable body, and reading it fails on Unix, so
/// the entry is rejected at the read stage either way. A publish step that
/// treated every such failure as a cache miss would report success after
/// destroying evidence of the fault.
#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache failure mismatches more clearly than errors"
)]
fn write_ir_cache_propagates_a_non_missing_read_failure(
    cache_fixture: Result<CacheFixture, Box<dyn std::error::Error>>,
) -> TestResult {
    let fixture = cache_fixture?;
    fixture.dir.create_dir("ir.json")?;

    let error = write_ir_cache(&fixture.paths, r#"{"ir_version":"1.0.0"}"#)
        .expect_err("an unreadable cache entry must not be overwritten");

    assert_eq!(
        error.stage(),
        PublishStage::Read,
        "an unreadable cache entry must be reported as a read failure"
    );
    assert!(
        matches!(error.into_source(), OrthohelpError::Io { .. }),
        "cache-read failure must keep its I/O error"
    );
    Ok(())
}

/// A large mismatched cache entry is replaced without being read.
///
/// The pre-existing file is several megabytes, while the new payload is a few
/// bytes short. Their lengths differ, so the comparison returns after the
/// metadata check; the `bounded_body_matches` tests above prove that the body
/// is not read in that case. Replacing the entry is the observable outcome.
#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache replacement mismatches more clearly than errors"
)]
fn write_ir_cache_replaces_a_large_mismatched_entry(
    cache_fixture: Result<CacheFixture, Box<dyn std::error::Error>>,
) -> TestResult {
    const SMALL: &str = r#"{"ir_version":"1.0.0"}"#;
    const LARGE_BYTES: usize = 4 * 1024 * 1024;

    let fixture = cache_fixture?;
    let large_entry = "x".repeat(LARGE_BYTES);
    fixture.dir.write("ir.json", &large_entry)?;
    assert_ne!(
        large_entry.len(),
        SMALL.len(),
        "the fixture must exercise the metadata-length mismatch branch"
    );

    assert_eq!(
        write_ir_cache(&fixture.paths, SMALL).expect("replace the large cache entry"),
        PublishOutcome::Updated,
        "a mismatched cache entry must be replaced rather than reused"
    );
    let written = fixture.dir.read_to_string("ir.json")?;

    assert_eq!(written, SMALL, "the cache entry must hold the new payload");
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report cache mtime mismatches more clearly than errors"
)]
fn write_ir_cache_is_idempotent(
    cache_fixture: Result<CacheFixture, Box<dyn std::error::Error>>,
) -> TestResult {
    const CONTENT: &str = r#"{"ir_version":"1.0.0"}"#;
    const OTHER: &str = r#"{"ir_version":"2.0.0"}"#;
    const MTIME_TIMEOUT: Duration = Duration::from_millis(1_500);

    let fixture = cache_fixture?;
    let bridge = &fixture.dir;

    assert_eq!(
        write_ir_cache(&fixture.paths, CONTENT).expect("first write"),
        PublishOutcome::Updated,
        "an absent cache entry must report that it was written"
    );
    let mtime1 = read_mtime(bridge, "ir.json")?;

    assert_eq!(
        write_ir_cache(&fixture.paths, CONTENT).expect("idempotent write"),
        PublishOutcome::Unchanged,
        "identical content must report that the cache was left untouched"
    );
    let mtime2 = poll_mtime_until(bridge, "ir.json", MTIME_TIMEOUT, |mtime| mtime == mtime1)?;

    assert_eq!(
        mtime1, mtime2,
        "mtime should not change when content is identical"
    );

    let next_filesystem_tick = mtime2 + Duration::from_secs(1);
    poll_clock_after(next_filesystem_tick, MTIME_TIMEOUT);
    assert_eq!(
        write_ir_cache(&fixture.paths, OTHER).expect("write new content"),
        PublishOutcome::Updated,
        "changed content must report that the cache was rewritten"
    );
    let mtime3 = poll_mtime_until(bridge, "ir.json", MTIME_TIMEOUT, |mtime| mtime > mtime2)?;

    assert!(mtime3 > mtime2, "mtime should advance when content changes");
    Ok(())
}
