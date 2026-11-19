//! `Cargo.lock` parsing and fail-closed selection.
//!
//! The lock records the resolution this workspace actually builds, so it is the
//! primary identity source: it carries the exact version, source and registry
//! checksum for every package. Selection refuses a missing or duplicated name
//! rather than taking the first match, because ADR-0028 requires an ambiguous
//! resolution to stop the gate instead of choosing silently.

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

/// A parsed `Cargo.lock` document.
///
/// Only the `package` table is modelled; the lock's `version` header and any
/// future tables are ignored, so a format addition cannot fail the gate for an
/// unrelated reason.
#[derive(Debug, Deserialize)]
pub(crate) struct Lockfile {
    #[serde(default)]
    package: Vec<LockedPackage>,
}

/// One `[[package]]` entry in `Cargo.lock`.
#[derive(Debug, Deserialize)]
pub(crate) struct LockedPackage {
    /// Resolved package name.
    pub(crate) name: String,
    /// Exact resolved version.
    pub(crate) version: String,
    /// Registry or other source identity; absent for path packages.
    #[serde(default)]
    pub(crate) source: Option<String>,
    /// Registry checksum; absent for path and git packages.
    #[serde(default)]
    pub(crate) checksum: Option<String>,
    /// Dependencies, each written `name`, `name version`, or with a source.
    #[serde(default)]
    pub(crate) dependencies: Vec<String>,
}

impl Lockfile {
    /// Parse a lockfile document, naming the source on failure.
    ///
    /// ```ignore
    /// let lock = Lockfile::parse(&read_text(&lock_path)?)?;
    /// ```
    pub(crate) fn parse(document: &str) -> Result<Self> {
        toml::from_str(document).context("the Cargo.lock document should be valid TOML")
    }

    /// Every package entry, in lockfile order.
    pub(crate) fn packages(&self) -> &[LockedPackage] {
        &self.package
    }
}

/// Select the only package named `name`.
///
/// A missing entry and a duplicated entry both fail: the gate must never fall
/// back to an arbitrary match when the resolution is not what was reviewed.
pub(crate) fn select_unique<'a>(
    packages: &'a [LockedPackage],
    name: &str,
) -> Result<&'a LockedPackage> {
    let mut matches = packages.iter().filter(|package| package.name == name);
    let Some(selected) = matches.next() else {
        bail!("Cargo.lock has no package named {name:?}");
    };
    ensure!(
        matches.next().is_none(),
        "Cargo.lock lists more than one package named {name:?}; \
         the gate refuses to choose an ambiguous resolution",
    );
    Ok(selected)
}

/// The package name in one lock dependency entry.
///
/// Entries may also carry a version or a source qualifier; the leading token is
/// always the name.
pub(crate) fn dependency_name(entry: &str) -> &str {
    entry.split_whitespace().next().unwrap_or(entry)
}
