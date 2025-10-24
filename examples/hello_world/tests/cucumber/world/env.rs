//! Environment management helpers for the Cucumber world.
use super::{ENV_PREFIX, World};
use anyhow::{Context, Result};
use std::fs;
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
        let base = self.workdir.path().join("xdg-config");
        let config_dir = base.join("hello_world");
        fs::create_dir_all(&config_dir).context("create XDG hello_world directory")?;
        fs::write(config_dir.join("hello_world.toml"), contents)
            .context("write XDG hello_world config")?;
        let value = base.to_string_lossy().into_owned();
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
