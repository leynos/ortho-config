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
//!
//! # Assumption
//!
//! These properties assume a file timestamp granularity finer than the poll
//! budget, which the filesystems this repository builds and tests on all
//! provide. An mtime is stamped when the write happens and never revised
//! afterwards, so two writes that land in the same timestamp bucket cannot be
//! told apart by any amount of polling: the later one's timestamp is already
//! fixed. The sibling `tests` module handles the same limit by waiting for the
//! system clock to clear a whole second before its replacement write, which is
//! affordable for its single literal pair. That wait is deliberately not
//! repeated here, because it would add about a second to each of 256 generated
//! cases to defend a case no supported filesystem exhibits.

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

/// How long to wait for a replacement's timestamp to diverge from the previous
/// one.
///
/// Reaching the sibling `tests` module's value, which allows for a filesystem
/// that places the next legal timestamp a full second away. Polling keeps the
/// usual wait at one interval, since the read that follows a replacement
/// normally differs at once.
const MTIME_TIMEOUT: Duration = Duration::from_millis(1_500);

/// How often the mtime poll re-reads the file.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

proptest! {
    /// Re-offering identical content leaves the file, and its mtime, alone.
    ///
    /// A rewrite moves the mtime and a correct implementation does not rewrite,
    /// so the two disagree at once; the read is therefore immediate, with no
    /// poll to wait out. The bytes are read as well, to catch a short circuit
    /// that leaves the document other than it found it. A rewrite of identical
    /// content writes identical bytes, so that second assertion passes under
    /// this mutation; the mtime is the signal for it, as `# Assumption` notes.
    #[test]
    fn rewriting_identical_content_preserves_mtime(content in generated_content()) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        let first = cache_mtime(&dir)?;

        write_cache(&paths, &content.original)?;
        prop_assert_eq!(
            cache_mtime(&dir)?,
            first,
            "rewriting an identical document moved the mtime of {}",
            content.original
        );
        prop_assert_eq!(
            cached_bytes(&dir)?,
            content.original.clone().into_bytes(),
            "an identical rewrite disturbed the cached document"
        );
    }

    /// Differing content replaces the file and moves its mtime forward.
    #[test]
    fn differing_content_advances_mtime_and_replaces_the_file(
        content in generated_content(),
    ) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        let first = cache_mtime(&dir)?;

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

    /// An identical write after a replacement leaves the newer file alone.
    ///
    /// This visits a state change the other properties do not: a cache that has
    /// already been replaced. An implementation that recognised identical
    /// content only while the file still held the *original* document would
    /// rewrite here, and the rewrite would be caught by the mtime and the bytes
    /// together. That the replacement itself moved the mtime is asserted by
    /// [`differing_content_advances_mtime_and_replaces_the_file`]; this property
    /// deliberately waits only on the second write, so it does not need to win
    /// the tick race that the first one sets up.
    #[test]
    fn an_identical_write_after_a_replacement_preserves_the_new_mtime(
        content in generated_content(),
    ) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        write_cache(&paths, &content.differing)?;
        let replaced = cache_mtime(&dir)?;

        write_cache(&paths, &content.differing)?;
        prop_assert_eq!(
            cache_mtime(&dir)?,
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
