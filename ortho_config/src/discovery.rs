//! Cross-platform configuration file discovery helpers.
//!
//! Applications can use [`ConfigDiscovery`] to enumerate configuration file
//! candidates in the same order exercised by the `hello_world` example. The
//! helper inspects explicit paths, XDG directories, Windows application data
//! folders, the user's home directory and project roots.
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{OrthoResult, load_config_file};

/// Builder for [`ConfigDiscovery`].
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::discovery::ConfigDiscovery;
///
/// # fn run() -> ortho_config::OrthoResult<()> {
/// let discovery = ConfigDiscovery::builder("hello_world")
///     .env_var("HELLO_WORLD_CONFIG_PATH")
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
}

impl ConfigDiscoveryBuilder {
    /// Creates a builder initialised for `app_name`.
    ///
    /// The `app_name` populates platform directories such as
    /// `$XDG_CONFIG_HOME/<app_name>/config.toml` and
    /// `%APPDATA%\<app_name>\config.toml`.
    #[must_use]
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            env_var: None,
            app_name: app_name.into(),
            config_file_name: String::from("config.toml"),
            custom_dotfile_name: None,
            custom_project_file_name: None,
            project_roots: vec![PathBuf::new()],
            explicit_paths: Vec::new(),
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

    fn default_dotfile(&self) -> String {
        let stem = self.app_name.trim();
        let extension = Path::new(&self.config_file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if stem.is_empty() {
            match extension {
                "" => String::from(".config"),
                ext => format!(".{ext}"),
            }
        } else if extension.is_empty() {
            format!(".{stem}")
        } else {
            format!(".{stem}.{extension}")
        }
    }

    /// Finalises the builder and returns a [`ConfigDiscovery`].
    #[must_use]
    pub fn build(self) -> ConfigDiscovery {
        let default_dotfile = self.default_dotfile();
        let dotfile_name = self.custom_dotfile_name.unwrap_or(default_dotfile);
        let project_file_name = self
            .custom_project_file_name
            .unwrap_or_else(|| dotfile_name.clone());

        ConfigDiscovery {
            env_var: self.env_var,
            explicit_paths: self.explicit_paths,
            app_name: self.app_name,
            config_file_name: self.config_file_name,
            dotfile_name,
            project_file_name,
            project_roots: self.project_roots,
        }
    }
}

/// Cross-platform configuration discovery helper mirroring the `hello_world` example.
#[derive(Debug, Clone)]
pub struct ConfigDiscovery {
    env_var: Option<String>,
    explicit_paths: Vec<PathBuf>,
    app_name: String,
    config_file_name: String,
    dotfile_name: String,
    project_file_name: String,
    project_roots: Vec<PathBuf>,
}

impl ConfigDiscovery {
    /// Creates a new builder initialised for `app_name`.
    #[must_use]
    pub fn builder(app_name: impl Into<String>) -> ConfigDiscoveryBuilder {
        ConfigDiscoveryBuilder::new(app_name)
    }

    fn push_unique(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, candidate: PathBuf) {
        if candidate.as_os_str().is_empty() {
            return;
        }
        if seen.insert(candidate.clone()) {
            paths.push(candidate);
        }
    }

    fn push_explicit(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        if let Some(env_var) = &self.env_var {
            if let Some(value) = std::env::var_os(env_var) {
                if !value.is_empty() {
                    Self::push_unique(paths, seen, PathBuf::from(value));
                }
            }
        }

        for path in &self.explicit_paths {
            Self::push_unique(paths, seen, path.clone());
        }
    }

    fn push_nested(&self, base: &Path, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        let dir = if self.app_name.is_empty() {
            base.to_path_buf()
        } else {
            base.join(&self.app_name)
        };
        Self::push_unique(paths, seen, dir.join(&self.config_file_name));
    }

    fn push_dotfile(&self, base: &Path, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        Self::push_unique(paths, seen, base.join(&self.dotfile_name));
    }

    fn push_xdg(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
            let dir = PathBuf::from(dir);
            self.push_nested(&dir, paths, seen);
            self.push_dotfile(&dir, paths, seen);
        }

        if let Some(dirs) = std::env::var_os("XDG_CONFIG_DIRS") {
            for dir in std::env::split_paths(&dirs) {
                self.push_nested(&dir, paths, seen);
                self.push_dotfile(&dir, paths, seen);
            }
        } else if cfg!(any(unix, target_os = "redox")) {
            let dir = Path::new("/etc/xdg");
            self.push_nested(dir, paths, seen);
            self.push_dotfile(dir, paths, seen);
        }
    }

    fn push_windows(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        for key in ["APPDATA", "LOCALAPPDATA"] {
            if let Some(dir) = std::env::var_os(key) {
                let dir = PathBuf::from(dir);
                self.push_nested(&dir, paths, seen);
                self.push_dotfile(&dir, paths, seen);
            }
        }
    }

    fn push_home(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        if let Some(home) = home {
            let home_path = PathBuf::from(&home);
            let config_dir = home_path.join(".config");
            self.push_nested(&config_dir, paths, seen);
            Self::push_unique(paths, seen, home_path.join(&self.dotfile_name));
        }
    }

    fn push_projects(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        for root in &self.project_roots {
            Self::push_unique(paths, seen, root.join(&self.project_file_name));
        }
    }

    /// Returns the ordered configuration candidates.
    #[must_use]
    pub fn candidates(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        let mut paths = Vec::new();

        self.push_explicit(&mut paths, &mut seen);
        self.push_xdg(&mut paths, &mut seen);
        self.push_windows(&mut paths, &mut seen);
        self.push_home(&mut paths, &mut seen);
        self.push_projects(&mut paths, &mut seen);

        paths
    }

    /// Loads the first available configuration file using [`load_config_file`].
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`](crate::OrthoError) if reading a candidate fails.
    pub fn load_first(&self) -> OrthoResult<Option<figment::Figment>> {
        for path in self.candidates() {
            match load_config_file(&path) {
                Ok(Some(figment)) => return Ok(Some(figment)),
                Ok(None) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde::Deserialize;
    use tempfile::TempDir;
    use test_helpers::env as test_env;

    fn clear_common_env() -> Vec<test_env::EnvVarGuard> {
        let mut guards = Vec::new();
        for key in [
            "HELLO_WORLD_CONFIG_PATH",
            "XDG_CONFIG_HOME",
            "XDG_CONFIG_DIRS",
            "APPDATA",
            "LOCALAPPDATA",
            "HOME",
            "USERPROFILE",
        ] {
            guards.push(test_env::remove_var(key));
        }
        guards
    }

    #[rstest]
    fn env_override_precedes_other_candidates() {
        let _guards = clear_common_env();
        let path = std::env::temp_dir().join("explicit.toml");
        let _env = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &path);

        let discovery = ConfigDiscovery::builder("hello_world")
            .env_var("HELLO_WORLD_CONFIG_PATH")
            .build();
        let candidates = discovery.candidates();
        assert_eq!(candidates.first(), Some(&path));
    }

    #[rstest]
    fn xdg_candidates_follow_explicit_paths() {
        let _guards = clear_common_env();
        let dir = TempDir::new().expect("xdg");
        let xdg_path = dir.path().join("hello_world");
        std::fs::create_dir_all(&xdg_path).expect("create xdg dir");
        let _home = test_env::set_var("XDG_CONFIG_HOME", dir.path());

        let discovery = ConfigDiscovery::builder("hello_world").build();
        let candidates = discovery.candidates();
        let expected_first = xdg_path.join("config.toml");
        let expected_second = dir.path().join(".hello_world.toml");
        assert_eq!(candidates.first(), Some(&expected_first));
        assert_eq!(candidates.get(1), Some(&expected_second));
    }

    #[rstest]
    fn project_roots_append_last() {
        let _guards = clear_common_env();
        let discovery = ConfigDiscovery::builder("hello_world")
            .clear_project_roots()
            .add_project_root("proj")
            .build();
        let candidates = discovery.candidates();
        assert_eq!(
            candidates.last(),
            Some(&PathBuf::from("proj/.hello_world.toml"))
        );
    }

    #[derive(Debug, Deserialize)]
    struct SampleConfig {
        value: bool,
    }

    #[rstest]
    fn load_first_reads_first_existing_file() {
        let _guards = clear_common_env();
        let dir = TempDir::new().expect("config dir");
        let file_dir = dir.path().join("hello_world");
        std::fs::create_dir_all(&file_dir).expect("create hello_world dir");
        let file = file_dir.join("config.toml");
        std::fs::write(&file, "value = true").expect("write config");
        let _xdg = test_env::set_var("XDG_CONFIG_HOME", dir.path());

        let discovery = ConfigDiscovery::builder("hello_world").build();
        let figment = discovery
            .load_first()
            .expect("load figment")
            .expect("figment present");
        let config: SampleConfig = figment.extract().expect("extract sample config");
        assert!(config.value);
    }
}
