//! Shared helpers for file module tests along with focused submodules.

use super::{canonicalise, normalise_cycle_key};
use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(super) mod extends_tests;
pub(super) mod normalise_tests;
pub(super) mod path_tests;
#[cfg(feature = "yaml")]
pub(super) mod yaml_tests;

pub(super) fn canonical_root_and_current() -> Result<(PathBuf, PathBuf)> {
    let root = canonicalise(Path::new("."))
        .map_err(|err| anyhow!(err.to_string()))
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
    figment::Jail::try_with(|j| {
        let (root, current) = canonical_root_and_current().map_err(|err| {
            // figment::Error currently only implements `From<String>`, so stringify the source.
            figment::Error::from(err.to_string())
        })?;
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        f(j, &root, &current, &mut visited, &mut stack).map_err(|err| {
            // figment::Error currently only implements `From<String>`, so stringify the source.
            figment::Error::from(err.to_string())
        })
    })
    .map_err(|err| anyhow!(err.to_string()))
}

pub(super) fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| {
        f(j).map_err(|err| {
            // figment::Error currently only implements `From<String>`, so stringify the source.
            figment::Error::from(err.to_string())
        })
    })
    .map_err(|err| anyhow!(err.to_string()))
}

pub(super) fn to_anyhow<T>(result: crate::OrthoResult<T>) -> Result<T> {
    result.map_err(|err| anyhow!(err.to_string()))
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
