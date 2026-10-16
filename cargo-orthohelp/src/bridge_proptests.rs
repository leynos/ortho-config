//! Property tests for `ir.json` cache writes and what they leave behind.
//!
//! `write_ir_cache` holds one rule with two halves. Re-offering content the
//! cache already holds must leave the file alone, mtime included, because cache
//! validity is decided on timestamps; offering different content must replace
//! the file and move its mtime on. The sibling `tests` module checks a single
//! pair of literal documents; these properties generate documents on both sides
//! of the comparison and vary how much writing preceded it, so a short circuit
//! keyed on the wrong comparison fails here.
//!
//! # Reading a timestamp that a fresh write may not move
//!
//! Two writes in quick succession can be stamped so that the second is not
//! observably later than the first, and nothing in the `write_ir_cache`
//! contract says the filesystem must stamp them differently. Where that
//! happens, "the mtime moved" is unobservable however long it is polled, and a
//! property that waits for an advance reports a fault in the write that was
//! never there. Windows CI did exactly that: a property that wrote and polled
//! for the timestamp to pass the previous one failed on every case, while its
//! sibling — which never compares two writes — passed.
//!
//! Each property therefore pushes the cache file's mtime a minute into the past
//! before the write it means to judge, and compares against the value the
//! filesystem stored. A write then moves the timestamp on by a minute, which no
//! granularity can hide and no rounding can collapse, and a skipped write
//! leaves it exactly where it was. The advance is still polled with a bound,
//! since a filesystem may publish the new timestamp a little after the write
//! returns; polling is what the sibling `tests` module does for its one literal
//! pair, and backdating saves it the second-long wait for a real clock tick.
//!
//! The two halves lean on each other. The differing-content property establishes
//! that this filesystem does surface a write's timestamp, which is what lets
//! the other two read an *unchanged* backdated value as evidence that nothing
//! was written, rather than as evidence that the write was invisible.
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

use std::time::{Duration, Instant, SystemTime};

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
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

/// How far into the past the cache file is pushed before a write whose effect
/// on the timestamp is under test.
///
/// Wide enough that no plausible timestamp granularity could merge the
/// backdated value with the one a fresh write receives.
const BACKDATE_BY: Duration = Duration::from_secs(60);

/// How long to keep re-reading the cache file before concluding that a write
/// did not move its mtime.
///
/// Reaching the sibling `tests` module's value, which allows for a filesystem
/// that places the next legal timestamp a full second away.
const MTIME_TIMEOUT: Duration = Duration::from_millis(1_500);

/// How often the mtime poll re-reads the file.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

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

/// Backdates the cache file's mtime, returning the value the filesystem stored.
///
/// Opening for write is what makes this portable: setting a timestamp needs a
/// handle permitted to write attributes, which a read-only handle does not
/// offer on Windows. The open asks to write but not to truncate, so the
/// document survives it, and opening alone does not stamp the file — the value
/// read back is therefore the one just set rather than a fresh write's.
///
/// The stored value is returned rather than the requested one, so a filesystem
/// that rounds timestamps to a coarser unit still gets compared against what it
/// actually holds.
fn backdate_cache_mtime(dir: &Dir) -> std::result::Result<SystemTime, TestCaseError> {
    let target = SystemTime::now().checked_sub(BACKDATE_BY).ok_or_else(|| {
        TestCaseError::fail("system clock is within a minute of the epoch; cannot backdate")
    })?;
    let file = dir
        .open_with("ir.json", OpenOptions::new().write(true))
        .map_err(|error| TestCaseError::fail(format!("open ir.json to backdate it: {error}")))?;
    file.into_std()
        .set_modified(target)
        .map_err(|error| TestCaseError::fail(format!("backdate ir.json mtime: {error}")))?;
    cache_mtime(dir)
}

/// Polls the cache file's mtime until `matches` accepts it, or reports failure.
///
/// Polling rather than sleeping a fixed interval keeps the common case — where
/// the filesystem has already published the write's timestamp — free of any
/// wait.
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

proptest! {
    /// Re-offering identical content leaves the file, and its mtime, alone.
    ///
    /// The mtime is compared against a backdated value, which is what gives a
    /// rewrite something to move: it writes identical bytes, so the byte
    /// comparison below has nothing to object to and the timestamp is the
    /// signal. Reading it straight away is deliberate — this property asserts
    /// that nothing happened, and waiting cannot make an absence more
    /// conclusive.
    #[test]
    fn rewriting_identical_content_preserves_mtime(content in generated_content()) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        let first = backdate_cache_mtime(&dir)?;

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
    ///
    /// This is also what shows the filesystem surfaces a write's timestamp at
    /// all, which the "nothing was written" readings elsewhere rely on.
    #[test]
    fn differing_content_advances_mtime_and_replaces_the_file(
        content in generated_content(),
    ) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        let first = backdate_cache_mtime(&dir)?;

        write_cache(&paths, &content.differing)?;
        let second = poll_mtime_until(&dir, |mtime| mtime > first)?;

        prop_assert!(
            second > first,
            "mtime did not advance past the backdated {first:?} for differing content"
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
    /// rewrite here, moving the timestamp off the backdated value and changing
    /// the bytes with it.
    #[test]
    fn an_identical_write_after_a_replacement_preserves_the_new_mtime(
        content in generated_content(),
    ) {
        let (_temp, dir, paths) = scratch_bridge()?;

        write_cache(&paths, &content.original)?;
        write_cache(&paths, &content.differing)?;
        let replaced = backdate_cache_mtime(&dir)?;

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
