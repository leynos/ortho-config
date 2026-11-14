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

// ---------------------------------------------------------------------------
// Fingerprinting failures
//
// A fingerprint that silently ignores an input it could not read would pin a
// stale IR to a fresh source tree, so every unreadable input has to surface as
// an error. The cases below force each arm of that rule with a filesystem
// state no healthy workspace produces.
// ---------------------------------------------------------------------------

/// A temporary workspace root, kept alive by the returned handle.
fn workspace() -> (tempfile::TempDir, Utf8PathBuf) {
    let tempdir = tempfile::tempdir().expect("temp dir");
    let root =
        Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf()).expect("temporary path is UTF-8");
    (tempdir, root)
}

/// The I/O error a fingerprinting failure was reported with.
fn io_failure(failure: OrthohelpError) -> (Utf8PathBuf, std::io::Error) {
    match failure {
        OrthohelpError::Io { path, source } => (path, source),
        other => panic!("fingerprinting reports an I/O error, not {other:?}"),
    }
}

#[test]
fn a_missing_workspace_root_is_an_error_for_the_lockfile() {
    let (_tempdir, root) = workspace();
    let absent = root.join("absent");

    let failure = lockfile_fingerprint(&absent)
        .expect_err("a workspace root that cannot be opened must not read as a missing lockfile");

    let (path, source) = io_failure(failure);
    assert_eq!(path, absent, "the failure names the root it could not open");
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn a_missing_package_root_is_an_error_for_the_fingerprint() {
    let (_tempdir, root) = workspace();
    let absent = root.join("absent");

    let failure = fingerprint_package(&absent)
        .expect_err("an unreadable package root must not fingerprint as an empty package");

    let (path, source) = io_failure(failure);
    assert_eq!(path, absent, "the failure names the root it could not open");
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn a_lockfile_that_cannot_be_opened_is_reported() {
    let (_tempdir, root) = workspace();
    // A link that resolves to itself fails to open on every account, which
    // distinguishes "present but unreadable" from "absent, so not an input".
    std::os::unix::fs::symlink("Cargo.lock", root.join("Cargo.lock"))
        .expect("create a self-referential link");

    let failure =
        lockfile_fingerprint(&root).expect_err("an unopenable lockfile must not read as absent");

    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        root.join("Cargo.lock"),
        "the failure names the lockfile it could not open"
    );
}

#[cfg(unix)]
#[test]
fn a_lockfile_that_cannot_be_read_is_reported() {
    let (_tempdir, root) = workspace();
    std::fs::create_dir(root.join("Cargo.lock")).expect("create a directory in the file's place");

    let failure =
        lockfile_fingerprint(&root).expect_err("an unreadable lockfile must not be fingerprinted");

    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        root.join("Cargo.lock"),
        "the failure names the lockfile it could not read"
    );
}

#[cfg(unix)]
#[test]
fn a_package_file_that_cannot_be_opened_is_reported() {
    let (_tempdir, root) = workspace();
    std::os::unix::fs::symlink("Cargo.toml", root.join("Cargo.toml"))
        .expect("create a self-referential link");

    let failure = fingerprint_package(&root)
        .expect_err("an unopenable package file must not be treated as absent");

    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        Utf8Path::new("Cargo.toml"),
        "the failure names the package file it could not open"
    );
}

#[cfg(unix)]
#[test]
fn a_package_file_that_cannot_be_read_is_reported() {
    let (_tempdir, root) = workspace();
    std::fs::create_dir(root.join("Cargo.toml")).expect("create a directory in the file's place");

    let failure =
        fingerprint_package(&root).expect_err("an unreadable package file must not be hashed");

    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        Utf8Path::new("Cargo.toml"),
        "the failure names the package file it could not read"
    );
}

#[cfg(unix)]
#[test]
fn a_source_tree_that_cannot_be_opened_is_reported() {
    let (_tempdir, root) = workspace();
    std::os::unix::fs::symlink("src", root.join("src")).expect("create a self-referential link");

    let failure =
        fingerprint_package(&root).expect_err("a source tree that cannot be opened is not absent");

    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        Utf8Path::new("src"),
        "the failure names the tree it could not open"
    );
}

#[cfg(unix)]
#[test]
fn a_source_entry_that_is_not_valid_utf8_is_reported() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let (_tempdir, root) = workspace();
    std::fs::create_dir(root.join("src")).expect("create the source tree");
    let unnamed = root
        .join("src")
        .as_std_path()
        .join(OsStr::from_bytes(b"raw-\xff"));
    std::fs::write(unnamed, b"x").expect("create a file whose name is not UTF-8");

    let failure = fingerprint_package(&root)
        .expect_err("an entry the digest cannot name must abort the walk");

    let (path, source) = io_failure(failure);
    assert_eq!(
        path,
        Utf8Path::new("src"),
        "the failure names the tree holding the unnamed entry"
    );
    assert_ne!(
        source.kind(),
        std::io::ErrorKind::NotFound,
        "an entry the digest cannot name aborts the walk rather than reading as absent"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_source_directory_aborts_the_walk() {
    use std::os::unix::fs::PermissionsExt;

    let (_tempdir, root) = workspace();
    let private = root.join("src").join("private");
    std::fs::create_dir_all(&private).expect("create a nested source directory");
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o000))
        .expect("make the directory unreadable");

    // Privileged accounts bypass permission bits, so the walk would enter the
    // directory and fingerprint it instead of failing. Detect that the way
    // `ortho_config`'s probe tests do — by asking whether the locked path is
    // still readable — rather than by inspecting the uid.
    let reaches_through_mode = std::fs::read_dir(&private).is_ok();
    // One `match` rather than a `let ... else` rebind: the workspace denies
    // `clippy::shadow_reuse`, which any shadowing rebind of the outcome trips.
    let reached = if reaches_through_mode {
        None
    } else {
        Some(fingerprint_package(&root))
    };

    // Restore access before asserting so the temporary tree can be removed.
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o755))
        .expect("restore directory access");
    let Some(attempted) = reached else {
        return;
    };
    let (path, source) =
        io_failure(attempted.expect_err(
            "a directory the walk cannot enter must abort fingerprinting, not be skipped",
        ));
    assert_eq!(
        path,
        Utf8Path::new("src/private"),
        "the failure names the directory it could not open"
    );
    assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
}
