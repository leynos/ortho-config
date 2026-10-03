//! Shared helpers for file module tests along with focused submodules.

use super::canonicalise;
use super::path::normalize_cycle_key;
use anyhow::{Context, Result};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use rstest::fixture;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

pub(super) mod extends_tests;
pub(super) mod normalise_tests;
pub(super) mod path_tests;
#[cfg(feature = "yaml")]
pub(super) mod yaml_tests;

fn canonical_root_and_current_with<F>(
    root_path: &Path,
    canonicalise_fn: F,
) -> Result<(PathBuf, PathBuf)>
where
    F: FnOnce(&Path) -> crate::OrthoResult<PathBuf>,
{
    let root = canonicalise_fn(root_path)
        .map_err(anyhow::Error::new)
        .context("canonicalise configuration root directory")?;
    let current = root.join("config.toml");
    Ok((root, current))
}

/// Per-test isolation root for file-module tests.
///
/// The [`TempDir`] is held rather than exposed so it lives for as long as the
/// fixture value does; dropping it would delete the directory that
/// [`Self::root`] points into. Bind the whole fixture — destructuring with
/// `..` drops the guard at the end of the `let` statement and deletes the root
/// before the test body runs.
///
/// Keeping the root canonical matters because
/// `load_config_file` canonicalises paths before reading them, so tests that
/// speak in `Dir`-relative names still observe absolute, symlink-resolved
/// paths in errors and merge results.
pub(super) struct TestRoot {
    _temp: TempDir,
    /// Canonical form of the temporary root.
    pub(super) root: PathBuf,
    /// Directory capability scoped to the temporary root, for fixture writes.
    pub(super) dir: Dir,
}

/// Fallible fixture returning a fresh isolation root for one test.
///
/// Consume it as `test_root: Result<TestRoot>` and bind the whole value
/// (`let root = test_root?;`) so the [`TempDir`] guard is not dropped early.
/// Renaming the binding also keeps the denied `shadow_*` lints satisfied.
#[fixture]
pub(super) fn test_root() -> Result<TestRoot> {
    let temp = TempDir::new().context("create isolated configuration directory")?;
    let dir = Dir::open_ambient_dir(temp.path(), ambient_authority())
        .context("open isolated configuration directory")?;
    let root = to_anyhow(canonicalise(temp.path()))?;
    Ok(TestRoot {
        _temp: temp,
        root,
        dir,
    })
}

/// Run `f` against a fresh isolation root and empty cycle-detection state.
///
/// The callback receives, in order, the root's [`Dir`], the canonical root,
/// the current `config.toml` path, and the mutable visited and stack graph
/// state that the extends loader threads through its cycle detection.
pub(super) fn with_fresh_graph<F>(f: F) -> Result<()>
where
    F: FnOnce(&Dir, &Path, &Path, &mut HashSet<PathBuf>, &mut Vec<PathBuf>) -> Result<()>,
{
    let test_root = test_root()?;
    let current = test_root.root.join("config.toml");
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    f(
        &test_root.dir,
        &test_root.root,
        &current,
        &mut visited,
        &mut stack,
    )
}

pub(super) fn to_anyhow<T>(result: crate::OrthoResult<T>) -> Result<T> {
    result.map_err(anyhow::Error::new)
}

pub(super) fn assert_normalize_cycle_key(
    windows_input: &str,
    windows_expected: &str,
    unix_input: &str,
    unix_expected: &str,
) -> Result<()> {
    let (input, expected) = if cfg!(windows) {
        (
            PathBuf::from(windows_input),
            PathBuf::from(windows_expected),
        )
    } else {
        (PathBuf::from(unix_input), PathBuf::from(unix_expected))
    };
    anyhow::ensure!(
        normalize_cycle_key(&input) == expected,
        "normalised path mismatch for input {input:?}"
    );
    Ok(())
}

#[derive(Debug)]
struct InnerError;

impl std::fmt::Display for InnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inner error")
    }
}

impl std::error::Error for InnerError {}

#[derive(Debug)]
struct OuterError {
    source: InnerError,
}

impl std::fmt::Display for OuterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "outer error")
    }
}

impl std::error::Error for OuterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

fn sample_file_error(path: &str) -> Arc<crate::OrthoError> {
    Arc::new(crate::OrthoError::File {
        path: PathBuf::from(path),
        source: Box::new(OuterError { source: InnerError }),
    })
}

#[test]
fn canonical_root_and_current_preserves_error_chain() -> Result<()> {
    let err =
        canonical_root_and_current_with(Path::new("."), |_| Err(sample_file_error("config.toml")))
            .expect_err("expected canonical_root_and_current to fail");
    let chain: Vec<String> = err.chain().map(ToString::to_string).collect();
    anyhow::ensure!(
        chain
            .iter()
            .any(|message| message.contains("canonicalise configuration root directory")),
        "expected context message in error chain, got {chain:?}"
    );
    anyhow::ensure!(
        chain.iter().any(|message| message == "outer error"),
        "expected outer error in chain, got {chain:?}"
    );
    anyhow::ensure!(
        chain.iter().any(|message| message.contains("config.toml")),
        "expected file path context in chain, got {chain:?}"
    );
    anyhow::ensure!(
        chain.iter().any(|message| message == "inner error"),
        "expected inner error in chain, got {chain:?}"
    );
    Ok(())
}

#[test]
fn to_anyhow_preserves_error_chain() -> Result<()> {
    let err = to_anyhow::<()>(Err(sample_file_error("config.toml")))
        .expect_err("expected to_anyhow to fail");
    let chain: Vec<String> = err.chain().map(ToString::to_string).collect();
    anyhow::ensure!(
        chain.iter().any(|message| message == "outer error"),
        "expected outer error in chain, got {chain:?}"
    );
    anyhow::ensure!(
        chain.iter().any(|message| message == "inner error"),
        "expected inner error in chain, got {chain:?}"
    );
    Ok(())
}
