//! Capability-scoped fixtures for the configuration inheritance tests.

use anyhow::Result;
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

/// Owns a temporary configuration tree and selects its file through an
/// injected discovery source. Tests can supply a separate merge environment.
pub struct ConfigFixture {
    root: TempDir,
    directory: Dir,
}

impl ConfigFixture {
    /// Create a fresh configuration tree.
    pub fn new() -> Result<Self> {
        let root = tempfile::tempdir()?;
        let directory = Dir::open_ambient_dir(root.path(), ambient_authority())?;
        Ok(Self { root, directory })
    }

    /// Resolve a fixture name to an absolute path without reading process cwd.
    pub fn path(&self, name: &str) -> std::path::PathBuf {
        self.root.path().join(name)
    }

    /// Write a fixture beneath the temporary directory capability.
    pub fn create_file(&self, name: impl AsRef<Path>, content: &str) -> Result<()> {
        self.directory.write(name, content)?;
        Ok(())
    }

    /// Create a directory beneath the temporary directory capability.
    pub fn create_dir(&self, name: impl AsRef<Path>) -> Result<()> {
        self.directory.create_dir(name)?;
        Ok(())
    }

    /// Load a derived configuration from the required fixture path and explicit environment.
    pub fn load<T, I, U>(
        &self,
        args: I,
        merge: ortho_config::MapEnv,
    ) -> ortho_config::OrthoResult<T>
    where
        T: ortho_config::OrthoConfig,
        I: IntoIterator<Item = U>,
        U: Into<OsString>,
    {
        let mut cli_args: Vec<OsString> = args.into_iter().map(Into::into).collect();
        cli_args.push(OsString::from("--config-path"));
        cli_args.push(self.path(".config.toml").into_os_string());
        let discovery =
            ortho_config::MapEnv::new().with_var("CONFIG_PATH", self.path(".config.toml"));
        T::load_from_iter_with_sources(cli_args, Arc::new(discovery), Arc::new(merge))
    }
}
