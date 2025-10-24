//! Environment management helpers for the Cucumber world.
use super::{ENV_PREFIX, World};
use anyhow::{Context, Result};
use cap_std::fs::OpenOptions;
use cap_std::{ambient_authority, fs::Dir};
use std::io::Write;
use tokio::process::Command;

impl World {
    /// Records an environment variable override scoped to the scenario.
    pub(crate) fn set_env<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.env.insert(key.into(), value.into());
    }

    /// Removes a scenario-scoped environment variable override.
    pub(crate) fn remove_env<S>(&mut self, key: S)
    where
        S: AsRef<str>,
    {
        self.env.remove(key.as_ref());
    }

    pub(crate) fn write_xdg_config_home(&mut self, contents: &str) -> Result<()> {
        let base_path = self.workdir.path().join("xdg-config");
        let work_dir = Dir::open_ambient_dir(self.workdir.path(), ambient_authority())
            .context("open hello_world workdir for XDG setup")?;
        work_dir
            .create_dir_all("xdg-config/hello_world")
            .context("create XDG hello_world directory")?;
        let mut file = work_dir
            .open_with(
                "xdg-config/hello_world/hello_world.toml",
                OpenOptions::new().write(true).create(true).truncate(true),
            )
            .context("open XDG hello_world config for write")?;
        file.write_all(contents.as_bytes())
            .context("write XDG hello_world config")?;
        let value = base_path.to_string_lossy().into_owned();
        self.set_env("XDG_CONFIG_HOME", value);
        Ok(())
    }

    pub(crate) fn configure_environment(&self, command: &mut Command) {
        Self::scrub_command_environment(command);
        for (key, value) in &self.env {
            command.env(key, value);
        }
    }

    fn scrub_command_environment(command: &mut Command) {
        for (key, _) in std::env::vars_os() {
            if key
                .to_str()
                .is_some_and(|name| name.starts_with(ENV_PREFIX))
            {
                command.env_remove(&key);
            }
        }
    }
}
