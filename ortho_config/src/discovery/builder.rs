//! Builder for configuration discovery helpers.
//!
//! The builder lets applications customise environment variables, filenames,
//! and project roots before producing a [`ConfigDiscovery`] instance that
//! drives the search order.

use std::path::{Path, PathBuf};

use super::ConfigDiscovery;

/// Builder for [`ConfigDiscovery`].
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::discovery::ConfigDiscovery;
///
/// # fn run() -> ortho_config::OrthoResult<()> {
/// let discovery = ConfigDiscovery::builder("hello_world")
///     .add_explicit_path("./hello_world.toml")
///     .build();
///
/// if let Some(figment) = discovery.load_first()? {
///     #[derive(serde::Deserialize)]
///     struct Greeting { recipient: String }
///     let config: Greeting = figment.extract()?;
///     println!("Loaded greeting for {}", config.recipient);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ConfigDiscoveryBuilder {
    env_var: Option<String>,
    app_name: String,
    config_file_name: String,
    custom_dotfile_name: Option<String>,
    custom_project_file_name: Option<String>,
    project_roots: Vec<PathBuf>,
    explicit_paths: Vec<PathBuf>,
    required_explicit_paths: Vec<PathBuf>,
}

impl ConfigDiscoveryBuilder {
    /// Creates a builder initialised for `app_name`.
    ///
    /// The `app_name` populates platform directories such as
    /// `$XDG_CONFIG_HOME/<app_name>/config.toml` and
    /// `%APPDATA%\\<app_name>\\config.toml`.
    #[must_use]
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            env_var: None,
            app_name: app_name.into(),
            config_file_name: String::from("config.toml"),
            custom_dotfile_name: None,
            custom_project_file_name: None,
            project_roots: Vec::new(),
            explicit_paths: Vec::new(),
            required_explicit_paths: Vec::new(),
        }
    }

    /// Sets the environment variable consulted for an explicit configuration path.
    #[must_use]
    pub fn env_var(mut self, env_var: impl Into<String>) -> Self {
        self.env_var = Some(env_var.into());
        self
    }

    /// Overrides the canonical configuration file name searched under platform directories.
    #[must_use]
    pub fn config_file_name(mut self, name: impl Into<String>) -> Self {
        self.config_file_name = name.into();
        self
    }

    /// Sets a custom dotfile name used in directories that search for hidden files.
    #[must_use]
    pub fn dotfile_name(mut self, name: impl Into<String>) -> Self {
        self.custom_dotfile_name = Some(name.into());
        self
    }

    /// Overrides the filename searched within project roots.
    #[must_use]
    pub fn project_file_name(mut self, name: impl Into<String>) -> Self {
        self.custom_project_file_name = Some(name.into());
        self
    }

    /// Removes all project roots from the builder.
    #[must_use]
    pub fn clear_project_roots(mut self) -> Self {
        self.project_roots.clear();
        self
    }

    /// Replaces the project roots searched for configuration files.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::discovery::ConfigDiscovery;
    ///
    /// let discovery = ConfigDiscovery::builder("hello_world")
    ///     .project_roots(["./workspace", "./fallback"])
    ///     .build();
    /// let candidates = discovery.candidates();
    /// assert!(candidates.ends_with(&[
    ///     std::path::PathBuf::from("./workspace/.hello_world.toml"),
    ///     std::path::PathBuf::from("./fallback/.hello_world.toml"),
    /// ]));
    /// ```
    #[must_use]
    pub fn project_roots<I, P>(mut self, roots: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.project_roots = roots.into_iter().map(Into::into).collect();
        self
    }

    /// Adds an additional project root searched for configuration files.
    #[must_use]
    pub fn add_project_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.project_roots.push(root.into());
        self
    }

    /// Adds an explicit candidate path that precedes platform discovery.
    #[must_use]
    pub fn add_explicit_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.explicit_paths.push(path.into());
        self
    }

    /// Adds an explicit candidate path that must exist.
    ///
    /// This is primarily used for CLI-specified paths where falling back to
    /// other discovery locations would be surprising.
    #[must_use]
    pub fn add_required_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.required_explicit_paths.push(path.into());
        self
    }

    fn default_dotfile(&self) -> String {
        let stem = self.app_name.trim();
        let extension = Path::new(&self.config_file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| !ext.is_empty());

        if stem.is_empty() {
            let mut name = String::from('.');
            name.push_str(extension.unwrap_or("config"));
            return name;
        }

        let mut name = String::from('.');
        name.push_str(stem);
        if let Some(ext) = extension {
            name.push('.');
            name.push_str(ext);
        }
        name
    }

    /// Finalises the builder and returns a [`ConfigDiscovery`].
    #[must_use]
    pub fn build(self) -> ConfigDiscovery {
        let default_dotfile = self.default_dotfile();
        let dotfile_name = self.custom_dotfile_name.unwrap_or(default_dotfile);
        let project_file_name = self
            .custom_project_file_name
            .unwrap_or_else(|| dotfile_name.clone());

        let mut project_roots = self.project_roots;
        if let (true, Ok(dir)) = (project_roots.is_empty(), std::env::current_dir()) {
            project_roots.push(dir);
        }

        ConfigDiscovery {
            env_var: self.env_var,
            explicit_paths: self.explicit_paths,
            required_explicit_paths: self.required_explicit_paths,
            app_name: self.app_name,
            config_file_name: self.config_file_name,
            dotfile_name,
            project_file_name,
            project_roots,
        }
    }
}
