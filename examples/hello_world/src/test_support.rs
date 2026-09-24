//! Capability-scoped configuration fixtures for `hello_world` unit tests.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, MapEnv, SharedEnvSource, SharedScanEnvSource};
use tempfile::TempDir;

use crate::cli::GlobalConfigSources;

/// An isolated directory used for file-discovery and composition tests.
pub(crate) struct ConfigFixture {
    root: TempDir,
}

impl ConfigFixture {
    /// Create a fresh, capability-scoped fixture root.
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            root: tempfile::tempdir().context("create configuration fixture")?,
        })
    }

    /// Return the root used by file-bound source-aware loaders.
    pub(crate) fn path(&self) -> &Path {
        self.root.path()
    }

    /// Write one relative fixture path and return its absolute path.
    pub(crate) fn write(&self, name: &str, contents: &str) -> Result<PathBuf> {
        let directory = Dir::open_ambient_dir(self.path(), ambient_authority())
            .context("open configuration fixture")?;
        directory
            .write(name, contents.as_bytes())
            .context("write configuration fixture")?;
        Ok(self.path().join(name))
    }

    /// Create a relative directory for an XDG or platform fallback fixture.
    pub(crate) fn create_dir(&self, name: &str) -> Result<()> {
        let directory = Dir::open_ambient_dir(self.path(), ambient_authority())
            .context("open configuration fixture")?;
        directory
            .create_dir_all(name)
            .context("create configuration fixture directory")?;
        Ok(())
    }

    /// Build discovery confined to this root and the supplied closed environment.
    pub(crate) fn discovery(&self, source: MapEnv) -> ConfigDiscovery {
        ConfigDiscovery::builder("hello_world")
            .env_var("HELLO_WORLD_CONFIG_PATH")
            .config_file_name("hello_world.toml")
            .dotfile_name(".hello_world.toml")
            .project_file_name(".hello_world.toml")
            .project_roots([self.path().to_path_buf()])
            .env_source(Arc::new(source))
            .build()
    }
}

/// Keep discovery lookup and configuration scanning explicit in test calls.
pub(crate) fn global_sources(discovery: MapEnv, merge: MapEnv) -> GlobalConfigSources {
    let discovery_source: SharedEnvSource = Arc::new(discovery);
    let merge_source: SharedScanEnvSource = Arc::new(merge);
    GlobalConfigSources::new(discovery_source, merge_source)
}
