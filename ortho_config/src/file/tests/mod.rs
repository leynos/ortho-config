//! Shared helpers for file module tests along with focused submodules.

use super::canonicalise;
use super::path::normalise_cycle_key;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use test_helpers::figment as figment_helpers;

pub(super) mod extends_tests;
pub(super) mod normalise_tests;
pub(super) mod path_tests;
#[cfg(feature = "yaml")]
pub(super) mod yaml_tests;

pub(super) fn canonical_root_and_current() -> Result<(PathBuf, PathBuf)> {
    canonical_root_and_current_with(canonicalise)
}

fn canonical_root_and_current_with<F>(canonicalise_fn: F) -> Result<(PathBuf, PathBuf)>
where
    F: FnOnce(&Path) -> crate::OrthoResult<PathBuf>,
{
    let root = canonicalise_fn(Path::new("."))
        .map_err(anyhow::Error::new)
        .context("canonicalise configuration root directory")?;
    let current = root.join("config.toml");
    Ok((root, current))
}

pub(super) fn with_fresh_graph<F>(f: F) -> Result<()>
where
    F: FnOnce(
        &mut figment::Jail,
        &Path,
        &Path,
        &mut HashSet<PathBuf>,
        &mut Vec<PathBuf>,
    ) -> Result<()>,
{
    figment_helpers::with_jail(|j| {
        let (root, current) =
            canonical_root_and_current().map_err(figment_helpers::figment_error)?;
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        f(j, &root, &current, &mut visited, &mut stack).map_err(figment_helpers::figment_error)
    })
}

pub(super) fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment_helpers::with_jail(|j| f(j).map_err(figment_helpers::figment_error))
}

pub(super) fn to_anyhow<T>(result: crate::OrthoResult<T>) -> Result<T> {
    result.map_err(anyhow::Error::new)
}

pub(super) fn assert_normalise_cycle_key(
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
        normalise_cycle_key(&input) == expected,
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
    let err = canonical_root_and_current_with(|_| Err(sample_file_error("config.toml")))
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
