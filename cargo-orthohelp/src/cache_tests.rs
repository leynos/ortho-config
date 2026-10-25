//! Tests for cache fingerprinting.

use super::*;
use cap_std::fs_utf8::OpenOptions;
use rstest::{fixture, rstest};
use std::io::Write;

/// Error type for cache test helpers; kept generic since these helpers
/// only surface diagnostics for test failures, not production callers.
type CacheTestError = Box<dyn std::error::Error>;

/// Well-known SHA-256 digest of `b"abc"`. Pinning a canonical vector keeps
/// the digest rendering honest: a self-consistent but wrongly ordered or
/// zero-truncated encoder would still satisfy change-detection assertions.
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report fingerprint mismatches more clearly than errors"
)]
fn fingerprint_changes_on_file_update(
    temp_root: Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError>,
) -> Result<(), CacheTestError> {
    let (_tempdir, root, dir) = temp_root?;
    dir.create_dir_all("src")?;

    write_file(&dir, "Cargo.toml", "[package]\nname = \"demo\"\n")?;
    write_file(&dir, "src/lib.rs", "pub fn demo() -> u32 { 1 }\n")?;

    let first = fingerprint_package(&root)?;
    write_file(&dir, "src/lib.rs", "pub fn demo() -> u32 { 2 }\n")?;
    let second = fingerprint_package(&root)?;

    assert_ne!(first, second, "fingerprint should change when files change");
    assert!(
        is_lowercase_hex(&first),
        "package fingerprint should render as lowercase hex: {first}"
    );
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report fingerprint mismatches more clearly than errors"
)]
fn lockfile_fingerprint_matches_known_digest_vector(
    temp_root: Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError>,
) -> Result<(), CacheTestError> {
    let (_tempdir, root, dir) = temp_root?;
    write_file(&dir, "Cargo.lock", "abc")?;

    let fingerprint = lockfile_fingerprint(&root)?;

    assert_eq!(fingerprint.as_deref(), Some(ABC_SHA256));
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report fingerprint mismatches more clearly than errors"
)]
fn lockfile_fingerprint_is_absent_without_a_lockfile(
    temp_root: Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError>,
) -> Result<(), CacheTestError> {
    let (_tempdir, root, _dir) = temp_root?;

    let fingerprint = lockfile_fingerprint(&root)?;

    assert_eq!(fingerprint, None, "absent lockfile should not fingerprint");
    Ok(())
}

#[rstest]
fn cache_key_hash_renders_lowercase_hex() {
    let hash = cache_key().hash();
    let digest = hash
        .strip_prefix(CACHE_KEY_VERSION_PREFIX)
        .expect("cache key should include its format version");

    assert!(
        is_lowercase_hex(digest),
        "cache key digest should be lowercase hex digits: {hash}"
    );
}

#[rstest]
fn cache_key_hash_uses_the_leading_sha256_bytes() {
    assert_eq!(
        cache_key().hash(),
        "v1-151fb1ee840f9a46d9843813b6877cda",
        "cache key should version and use the leading digest bytes for its directory name"
    );
}

#[rstest]
fn cache_key_hash_names_a_directory_no_longer_than_the_digest_budget() {
    // The hash is a path component, and from cargo 1.100 every artefact
    // sits under `target/debug/build/<crate>/<hash>/out/`. Windows still
    // enforces a 260-character MAX_PATH on the names `link.exe` opens, so
    // this length is what keeps the longest fixture crate inside it.
    let hash = cache_key().hash();

    assert_eq!(
        hash.len(),
        CACHE_KEY_VERSION_PREFIX.len() + CACHE_DIRECTORY_DIGEST_BYTES * 2,
        "cache key should use the version prefix and {} hex digits: {hash}",
        CACHE_DIRECTORY_DIGEST_BYTES * 2
    );
}

#[rstest]
#[case::fingerprint(CacheKey { fingerprint: "other".to_owned(), ..cache_key() })]
#[case::root_type(CacheKey { root_type: "workspace".to_owned(), ..cache_key() })]
#[case::tool_version(CacheKey { tool_version: "0.9.0".to_owned(), ..cache_key() })]
#[case::ir_version(CacheKey { ir_version: "2".to_owned(), ..cache_key() })]
#[case::lockfile_hash(CacheKey { lockfile_hash: None, ..cache_key() })]
fn cache_key_hash_tracks_every_input(#[case] varied: CacheKey) {
    assert_ne!(
        cache_key().hash(),
        varied.hash(),
        "hash should change when any input changes"
    );
}

/// Baseline cache key whose fields each differ, so a hash that ignores or
/// transposes one field is detectable.
fn cache_key() -> CacheKey {
    CacheKey {
        fingerprint: "fingerprint".to_owned(),
        root_type: "package".to_owned(),
        tool_version: "0.8.0".to_owned(),
        ir_version: "1".to_owned(),
        lockfile_hash: Some("lockfile".to_owned()),
    }
}

/// Reports whether `value` is a lowercase hexadecimal string.
fn is_lowercase_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit))
}

#[fixture]
fn temp_root() -> Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError> {
    let tempdir = tempfile::tempdir()?;
    let root = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .map_err(|path| format!("tempdir path is not UTF-8: {}", path.display()))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
    Ok((tempdir, root, dir))
}

fn write_file(dir: &Dir, path: &str, contents: &str) -> Result<(), CacheTestError> {
    let mut file = dir.open_with(
        path,
        OpenOptions::new().write(true).create(true).truncate(true),
    )?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}
