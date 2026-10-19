//! Capability-scoped fixtures for the configuration inheritance tests.

use anyhow::{Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::ffi::OsString;
use std::sync::Arc;
use tempfile::TempDir;

/// Owns a temporary configuration tree and selects its file through an
/// injected discovery source. Tests can supply a separate merge environment.
pub struct ConfigFixture {
    _temporary_directory: TempDir,
    root_path: Utf8PathBuf,
    directory: Dir,
}

impl ConfigFixture {
    /// Create a fresh configuration tree.
    pub fn new() -> Result<Self> {
        let root = tempfile::tempdir()?;
        let root_path = Utf8PathBuf::from_path_buf(root.path().to_path_buf()).map_err(|path| {
            anyhow!(
                "temporary configuration path is not UTF-8: {}",
                path.display()
            )
        })?;
        let directory = Dir::open_ambient_dir(&root_path, ambient_authority())?;
        Ok(Self {
            _temporary_directory: root,
            root_path,
            directory,
        })
    }

    /// Resolve a fixture name to an absolute path without reading process cwd.
    pub fn path(&self, name: &str) -> Utf8PathBuf {
        self.root_path.join(name)
    }

    /// Write a fixture beneath the temporary directory capability.
    pub fn create_file(&self, name: impl AsRef<Utf8Path>, content: &str) -> Result<()> {
        self.directory.write(name, content)?;
        Ok(())
    }

    /// Create a directory beneath the temporary directory capability.
    pub fn create_dir(&self, name: impl AsRef<Utf8Path>) -> Result<()> {
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
