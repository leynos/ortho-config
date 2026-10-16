//! Property tests for `ir.json` cache writes and the mtime they preserve.
//!
//! `write_ir_cache` holds one rule with two halves. Re-offering content the
//! cache already holds must leave the file alone, mtime included, because cache
//! validity is decided on timestamps; offering different content must replace
//! the file and move its mtime forward. The sibling `tests` module checks a
//! single pair of literal documents; these properties generate documents on
//! both sides of the comparison and vary how much writing preceded it, so a
//! short circuit keyed on the wrong comparison fails here.
//!
//! # Scope
//!
//! Generated documents are JSON objects carrying their generator seed, which
//! makes the map from seed to text injective. That matters: the properties need
//! documents that are *known* to differ, and a generator with collisions would
//! assert "different content replaces the file" against a document the cache
//! had every right to treat as identical. The document shape itself is not
//! under test — the write path treats the text as opaque bytes — so a JSON
//! object is a readable stand-in for the real IR payload rather than a schema
//! the properties depend on.
//!
//! The wall-clock cost sits in the cases that replace the document, which must
//! wait for the filesystem to timestamp a new write. That is inherent to the
//! behaviour rather than to the generator: no choice of inputs makes a later
//! timestamp available sooner. The wait is therefore polled with a bound, and
//! exceeding the bound is reported as a test failure rather than a panic.

use std::time::{Duration, Instant, SystemTime};

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use tempfile::tempdir;

use super::*;

/// A generated `ir.json` document along with a second document that differs.
#[derive(Clone, Debug)]
struct GeneratedContent {
    /// The document written first, standing for pre-existing cache content.
    original: String,
    /// A document known to differ from [`Self::original`].
    differing: String,
}

/// The leading members a seed selects, from zero through seven.
///
/// The seed's low three bits are masked rather than reduced modulo eight, so
/// the shape varies without a division or remainder. Both operators are denied
/// across the workspace, and the mask gives exactly the same range here.
fn leading_member_count(seed: u64) -> u64 {
    seed & 0b111
}

/// Renders the document belonging to `seed`.
///
/// The seed appears as a member value, so distinct seeds give distinct text.
/// The leading members vary only the document's shape, and are always followed
/// by the seed member, so the document keeps at least one member and stays
/// well-formed at every seed — including the zero that shrinking reaches.
fn json_document(seed: u64) -> String {
    let mut members: Vec<String> = (0..leading_member_count(seed))
        .map(|index| format!("\"k{index}\":{index}"))
        .collect();
    members.push(format!("\"seed\":{seed}"));
    format!("{{{}}}", members.join(","))
}

/// Two documents drawn from disjoint seed ranges, so their text always differs.
fn generated_content() -> impl Strategy<Value = GeneratedContent> {
    (0u64..100_000, 0u64..100_000).prop_map(|(left, right)| GeneratedContent {
        original: json_document(left),
        differing: json_document(right + 1_000_000),
    })
}

/// A scratch bridge root, which is all `write_ir_cache` needs to exist.
///
/// The directory guard comes back with the paths because dropping it removes
/// the tree those paths point at.
fn scratch_bridge() -> std::result::Result<(tempfile::TempDir, Dir, BridgePaths), TestCaseError> {
    let fail = |error: std::io::Error| {
        TestCaseError::fail(format!("prepare scratch bridge root: {error}"))
    };
    let temp = tempdir().map_err(fail)?;
    let bridge_dir = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|path| {
        TestCaseError::fail(format!(
            "scratch bridge path is not UTF-8: {}",
            path.display()
        ))
    })?;
    let dir = Dir::open_ambient_dir(&bridge_dir, ambient_authority()).map_err(fail)?;
    let paths = BridgePaths {
        manifest_path: bridge_dir.join("Cargo.toml"),
        target_dir: bridge_dir.join("target"),
        ir_path: bridge_dir.join("ir.json"),
        bridge_dir,
    };
    Ok((temp, dir, paths))
}

/// The modification time of the cache file.
fn cache_mtime(dir: &Dir) -> std::result::Result<SystemTime, TestCaseError> {
    dir.metadata("ir.json")
        .and_then(|metadata| metadata.modified())
        .map(cap_std::time::SystemTime::into_std)
        .map_err(|error| TestCaseError::fail(format!("read ir.json mtime: {error}")))
}

/// The bytes currently stored in the cache file.
fn cached_bytes(dir: &Dir) -> std::result::Result<Vec<u8>, TestCaseError> {
    dir.read("ir.json")
        .map_err(|error| TestCaseError::fail(format!("read ir.json contents: {error}")))
}

/// Polls the cache file's mtime until `matches` accepts it, or reports failure.
///
/// Polling rather than sleeping a fixed interval keeps the common case — where
/// the filesystem has already timestamped the write — free of any wait.
fn poll_mtime_until(
    dir: &Dir,
    matches: impl Fn(SystemTime) -> bool,
) -> std::result::Result<SystemTime, TestCaseError> {
    let deadline = Instant::now() + MTIME_TIMEOUT;
    let mut mtime = cache_mtime(dir)?;
    while !matches(mtime) {
        if Instant::now() >= deadline {
            return Err(TestCaseError::fail(format!(
                "ir.json mtime did not reach the expected value before timeout: {mtime:?}"
            )));
        }
        std::thread::sleep(POLL_INTERVAL);
        mtime = cache_mtime(dir)?;
    }
    Ok(mtime)
}

/// Writes `json` to the cache, reporting any failure as a test case error.
fn write_cache(paths: &BridgePaths, json: &str) -> std::result::Result<(), TestCaseError> {
    write_ir_cache(paths, json)
        .map_err(|error| TestCaseError::fail(format!("write ir.json cache: {error:?}")))
}

/// How long to wait for the filesystem to timestamp a write.
///
/// Generous, because a coarse timestamp granularity can place the next legal
/// value a full second away, while polling keeps the wait near zero whenever
/// the filesystem has already moved on.
const MTIME_TIMEOUT: Duration = Duration::from_millis(1_500);

/// How often the mtime poll re-reads the file.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

proptest! {
    /// Re-offering identical content leaves the file, and its mtime, alone.
    ///
    /// The mtime is asserted stable twice. The immediate read catches a rewrite
    /// that crossed a timestamp tick, which is the case a filesystem with fine
    /// granularity reveals at once — and which the sibling unit test's single
    /// literal pair can miss for the same reason. The poll that follows then
    /// holds the property on a filesystem whose metadata settles late, so
    /// neither shape of clock can let a rewrite through unnoticed.
    #[test]
    fn rewriting_identical_content_preserves_mtime(content in generated_content()) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        let first = poll_mtime_until(&dir, |_| true)?;

        write_cache(&paths, &content.original)?;
        let immediately_after = cache_mtime(&dir)?;
        prop_assert_eq!(
            immediately_after,
            first,
            "rewriting an identical document moved the mtime of {}",
            content.original
        );

        let settled = poll_mtime_until(&dir, |mtime| mtime == first)?;
        prop_assert_eq!(
            settled,
            first,
            "mtime settled away from its original value for an identical rewrite"
        );
    }

    /// Differing content replaces the file and moves its mtime forward.
    #[test]
    fn differing_content_advances_mtime_and_replaces_the_file(
        content in generated_content(),
    ) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        let first = poll_mtime_until(&dir, |_| true)?;

        write_cache(&paths, &content.differing)?;
        let second = poll_mtime_until(&dir, |mtime| mtime > first)?;

        prop_assert!(
            second > first,
            "mtime did not advance from {first:?} for differing content"
        );
        prop_assert_eq!(
            cached_bytes(&dir)?,
            content.differing.clone().into_bytes(),
            "cache file does not hold the differing document"
        );
    }

    /// An identical write after a replacement preserves the newer mtime.
    ///
    /// This composes both halves across a state change the other properties
    /// leave unvisited: the replacement must advance the mtime, and the write
    /// after it must recognise the replacement and leave it alone. A short
    /// circuit still comparing against the pre-replacement content would rewrite
    /// here and move the mtime a second time.
    #[test]
    fn an_identical_write_after_a_replacement_preserves_the_new_mtime(
        content in generated_content(),
    ) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        write_cache(&paths, &content.differing)?;
        let replaced = poll_mtime_until(&dir, |_| true)?;

        write_cache(&paths, &content.differing)?;
        let settled = poll_mtime_until(&dir, |mtime| mtime == replaced)?;

        prop_assert_eq!(
            settled,
            replaced,
            "an identical rewrite moved the mtime of a replaced cache"
        );
        prop_assert_eq!(
            cached_bytes(&dir)?,
            content.differing.clone().into_bytes(),
            "the replacement did not survive the identical rewrite"
        );
    }
}
