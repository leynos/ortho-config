//! Private generated bridge inputs with independently selected compiler state.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

use super::{BridgeCache, BridgeConfig, BridgePaths};
use crate::error::OrthohelpError;

/// Keep generated inputs and the executed snapshot alive for one CLI invocation.
pub(super) struct BridgeWorkspace {
    /// Private generated manifest, source, and executable snapshot paths.
    pub(super) paths: BridgePaths,
    /// Temporary-directory owner that keeps generated files alive.
    _owner: TempDir,
}

impl BridgeWorkspace {
    /// Allocate private files while sharing only Cargo's selected target.
    pub(super) fn new(cache: &BridgeCache) -> Result<Self, OrthohelpError> {
        Dir::create_ambient_dir_all(&cache.cache_dir, ambient_authority()).map_err(|source| {
            OrthohelpError::Io {
                path: cache.cache_dir.clone(),
                source,
            }
        })?;
        let owner = tempfile::Builder::new()
            .prefix("invocation-")
            .tempdir_in(cache.cache_dir.as_std_path())
            .map_err(|source| OrthohelpError::Io {
                path: cache.cache_dir.clone(),
                source,
            })?;
        let bridge_dir =
            Utf8PathBuf::from_path_buf(owner.path().to_path_buf()).map_err(|path| {
                OrthohelpError::Message(format!("non-UTF-8 bridge path: {}", path.display()))
            })?;
        Ok(Self {
            paths: BridgePaths {
                manifest_path: bridge_dir.join("Cargo.toml"),
                bridge_dir,
                target_dir: cache.target_dir.clone(),
            },
            _owner: owner,
        })
    }

    /// Seed dependency versions from the selected workspace, without editing it.
    ///
    /// Cargo adds the generated root package to this private lockfile. Existing
    /// dependency versions remain reusable instead of resolving newer versions
    /// independently for every temporary bridge manifest.
    pub(super) fn seed_lockfile(&self, config: &BridgeConfig) -> Result<(), OrthohelpError> {
        let source_directory = Dir::open_ambient_dir(&config.workspace_root, ambient_authority())
            .map_err(|source| OrthohelpError::Io {
            path: config.workspace_root.clone(),
            source,
        })?;
        if !source_directory
            .try_exists("Cargo.lock")
            .map_err(|source| OrthohelpError::Io {
                path: config.workspace_root.join("Cargo.lock"),
                source,
            })?
        {
            return Ok(());
        }
        let destination = Dir::open_ambient_dir(&self.paths.bridge_dir, ambient_authority())
            .map_err(|source| OrthohelpError::Io {
                path: self.paths.bridge_dir.clone(),
                source,
            })?;
        source_directory
            .copy("Cargo.lock", &destination, "Cargo.lock")
            .map_err(|source| OrthohelpError::Io {
                path: self.paths.bridge_dir.join("Cargo.lock"),
                source,
            })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "bridge_workspace_tests.rs"]
mod tests;
