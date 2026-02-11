//! Tests covering discovery candidate deduplication behaviour.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::tempdir;

use super::*;

#[cfg(windows)]
fn canonicalish(path: &Path) -> PathBuf {
    match dunce::canonicalize(path) {
        Ok(p) => p,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(err) => panic!("failed to canonicalise {path:?}: {err}"),
    }
}

#[cfg(not(windows))]
fn canonicalish(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn assert_first_error_path(errors: &[Arc<OrthoError>], expected: &Path) {
    let err = errors
        .first()
        .expect("expected at least one error when asserting path");
    let path = match err.as_ref() {
        OrthoError::File { path, .. } => path,
        other => panic!("expected OrthoError::File, got {other:?}"),
    };
    assert_eq!(canonicalish(path), canonicalish(expected));
}

#[test]
fn load_first_partitioned_dedups_required_paths() {
    let dir = tempdir().expect("create tempdir");
    let required = dir.path().join("missing.toml");
    let optional = dir.path().join("optional.toml");
    fs::write(&optional, "invalid = {").expect("write invalid optional config");
    let discovery = ConfigDiscovery::builder("app")
        .add_required_path(&required)
        .add_required_path(&required)
        .add_explicit_path(&optional)
        .build();

    let outcome = discovery.load_first_partitioned();
    assert!(outcome.figment.is_none());
    assert_eq!(outcome.required_errors.len(), 1);
    assert_eq!(outcome.optional_errors.len(), 1);

    assert_first_error_path(&outcome.required_errors, &required);
    assert_first_error_path(&outcome.optional_errors, &optional);
}

#[cfg(windows)]
#[test]
fn normalised_key_lowercases_ascii_and_backslashes() {
    let key = ConfigDiscovery::normalised_key(Path::new("C:/Config/FILE.TOML"));
    assert_eq!(key, "c:\\config\\file.toml");
}

#[cfg(windows)]
#[test]
fn normalised_key_handles_unicode_case() {
    let key = ConfigDiscovery::normalised_key(Path::new("C:/Temp/CAFÉ.toml"));
    assert_eq!(key, "c:\\temp\\café.toml");
}
