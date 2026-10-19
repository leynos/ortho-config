//! Capability-scoped configuration fixtures for `hello_world` unit tests.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, MapEnv, SharedEnvSource, SharedScanEnvSource};
use rstest::fixture;
use tempfile::TempDir;

use crate::cli::{
    GlobalArgs, GlobalConfigSources, GreetCommand, HelloWorldCli, load_global_config_with_sources,
    load_greet_defaults_with_sources,
};

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

/// Load the shipped sample configuration through a closed environment.
///
/// The sample files under `config/` are read through a capability scoped to
/// the manifest directory, copied into a fresh [`ConfigFixture`], and loaded
/// through the source-aware global and greeting-default loaders. Nothing here
/// consults the process environment or the working directory.
///
/// The returned pair is everything a message-planning test needs to compose a
/// plan from the sample layering: the fully merged [`HelloWorldCli`] and the
/// greeting defaults resolved for its subcommand.
///
/// # Examples
///
/// ```ignore
/// #[rstest]
/// fn recipient_comes_from_sample(sample_config: SampleConfigFixture) -> Result<()> {
///     let (config, _greet) = sample_config?;
///     assert_eq!(config.recipient, "Excited crew");
///     Ok(())
/// }
/// ```
#[fixture]
pub(crate) fn sample_config() -> Result<(HelloWorldCli, GreetCommand)> {
    let fixture = ConfigFixture::new()?;
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_dir = Dir::open_ambient_dir(
        manifest_dir.join("config").as_std_path(),
        ambient_authority(),
    )
    .context("open sample configuration")?;
    let baseline = config_dir.read_to_string("baseline.toml")?;
    let overrides = config_dir.read_to_string("overrides.toml")?;
    fixture.write("baseline.toml", &baseline)?;
    let selected = fixture.write(".hello_world.toml", &overrides)?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected);
    let config = load_global_config_with_sources(
        &GlobalArgs::default(),
        None,
        "hello-world",
        global_sources(discovery.clone(), MapEnv::new()),
    )?;
    let greet =
        load_greet_defaults_with_sources(fixture.path(), global_sources(discovery, MapEnv::new()))?;
    Ok((config, greet))
}
