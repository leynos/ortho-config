//! Cross-platform configuration file discovery helpers.
//!
//! Applications can use [`ConfigDiscovery`] to enumerate configuration file
//! candidates in the same order exercised by the `hello_world` example. The
//! helper inspects explicit paths, XDG directories, Windows application data
//! folders, the user's home directory and project roots.
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use dirs::home_dir;

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
            project_roots: Vec::new(),
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

        let mut project_roots = self.project_roots;
        if project_roots.is_empty() {
            if let Ok(current_dir) = std::env::current_dir() {
                project_roots.push(current_dir);
            }
        }

        ConfigDiscovery {
            env_var: self.env_var,
            explicit_paths: self.explicit_paths,
            app_name: self.app_name,
            config_file_name: self.config_file_name,
            dotfile_name,
            project_file_name,
            project_roots,
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

    fn push_unique(paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>, candidate: PathBuf) {
        if candidate.as_os_str().is_empty() {
            return;
        }
        let key = Self::normalised_key(&candidate);
        if seen.insert(key) {
            paths.push(candidate);
        }
    }

    fn normalised_key(path: &Path) -> String {
        #[cfg(windows)]
        {
            path.to_string_lossy().to_lowercase()
        }

        #[cfg(not(windows))]
        {
            path.to_string_lossy().into_owned()
        }
    }

    fn push_explicit(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        if let Some(env_var) = &self.env_var {
            if let Some(value) = std::env::var_os(env_var).filter(|v| !v.is_empty()) {
                Self::push_unique(paths, seen, PathBuf::from(value));
            }
        }

        for path in &self.explicit_paths {
            Self::push_unique(paths, seen, path.clone());
        }
    }

    fn push_for_bases<I>(&self, bases: I, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>)
    where
        I: IntoIterator,
        I::Item: Into<PathBuf>,
    {
        for base in bases {
            let base = base.into();
            let nested = if self.app_name.is_empty() {
                base.clone()
            } else {
                base.join(&self.app_name)
            };
            Self::push_unique(paths, seen, nested.join(&self.config_file_name));
            Self::push_unique(paths, seen, base.join(&self.dotfile_name));
        }
    }

    fn push_xdg(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
            self.push_for_bases(std::iter::once(PathBuf::from(dir)), paths, seen);
        }

        if let Some(dirs) = std::env::var_os("XDG_CONFIG_DIRS") {
            self.push_for_bases(std::env::split_paths(&dirs), paths, seen);
        } else if cfg!(any(unix, target_os = "redox")) {
            self.push_for_bases(std::iter::once(PathBuf::from("/etc/xdg")), paths, seen);
        }
    }

    fn push_windows(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        let dirs = ["APPDATA", "LOCALAPPDATA"]
            .into_iter()
            .filter_map(|key| std::env::var_os(key).map(PathBuf::from));
        self.push_for_bases(dirs, paths, seen);
    }

    fn push_home(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .or_else(home_dir);
        if let Some(home_path) = home {
            let config_dir = home_path.join(".config");
            self.push_for_bases(std::iter::once(config_dir), paths, seen);
            Self::push_unique(paths, seen, home_path.join(&self.dotfile_name));
        }
    }

    fn push_projects(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        for root in &self.project_roots {
            Self::push_unique(paths, seen, root.join(&self.project_file_name));
        }
    }

    /// Returns the ordered configuration candidates.
    #[must_use]
    pub fn candidates(&self) -> Vec<PathBuf> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut paths = Vec::new();

        self.push_explicit(&mut paths, &mut seen);
        self.push_xdg(&mut paths, &mut seen);
        self.push_windows(&mut paths, &mut seen);
        self.push_home(&mut paths, &mut seen);
        self.push_projects(&mut paths, &mut seen);

        paths
    }

    /// Returns the ordered configuration candidates as [`Utf8PathBuf`] values.
    ///
    /// Paths that cannot be represented as UTF-8 are omitted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::ConfigDiscovery;
    ///
    /// std::env::set_var("HELLO_WORLD_CONFIG_PATH", "./hello_world.toml");
    /// let discovery = ConfigDiscovery::builder("hello_world")
    ///     .env_var("HELLO_WORLD_CONFIG_PATH")
    ///     .build();
    /// let mut utf8_candidates = discovery.utf8_candidates();
    /// assert_eq!(
    ///     utf8_candidates.remove(0),
    ///     camino::Utf8PathBuf::from("./hello_world.toml")
    /// );
    /// std::env::remove_var("HELLO_WORLD_CONFIG_PATH");
    /// ```
    #[must_use]
    pub fn utf8_candidates(&self) -> Vec<Utf8PathBuf> {
        self.candidates()
            .into_iter()
            .filter_map(|path| Utf8PathBuf::from_path_buf(path).ok())
            .collect()
    }

    /// Loads the first available configuration file using [`load_config_file`].
    ///
    /// # Behaviour
    ///
    /// Skips candidates that fail to load and continues scanning until an
    /// existing configuration file is parsed successfully.
    ///
    /// # Errors
    ///
    /// Currently always returns `Ok`; the [`OrthoResult`] return type keeps the
    /// API aligned with other loaders and reserves space for future failures.
    pub fn load_first(&self) -> OrthoResult<Option<figment::Figment>> {
        for path in self.candidates() {
            match load_config_file(&path) {
                Ok(Some(figment)) => return Ok(Some(figment)),
                Ok(None) => {}
                Err(_err) => {}
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

    fn setup_env_override_discovery() -> (
        ConfigDiscovery,
        PathBuf,
        Vec<test_env::EnvVarGuard>,
        test_env::EnvVarGuard,
    ) {
        let guards = clear_common_env();
        let path = std::env::temp_dir().join("explicit.toml");
        let env_guard = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &path);
        let discovery = ConfigDiscovery::builder("hello_world")
            .env_var("HELLO_WORLD_CONFIG_PATH")
            .build();

        (discovery, path, guards, env_guard)
    }

    #[rstest]
    fn env_override_precedes_other_candidates() {
        let (discovery, path, _guards, _env) = setup_env_override_discovery();
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
    fn utf8_candidates_prioritise_env_paths() {
        let (discovery, path, _guards, _env) = setup_env_override_discovery();
        let mut candidates = discovery.utf8_candidates();
        assert_eq!(
            candidates.remove(0),
            Utf8PathBuf::from_path_buf(path).expect("utf8 explicit path")
        );
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

    #[rstest]
    fn load_first_skips_invalid_candidates() {
        let _guards = clear_common_env();
        let dir = TempDir::new().expect("config dir");
        let invalid = dir.path().join("broken.toml");
        let valid = dir.path().join("valid.toml");
        std::fs::write(&invalid, "value = ???").expect("write invalid config");
        std::fs::write(&valid, "value = false").expect("write valid config");
        let _env = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &invalid);

        let discovery = ConfigDiscovery::builder("hello_world")
            .env_var("HELLO_WORLD_CONFIG_PATH")
            .add_explicit_path(valid.clone())
            .build();

        let figment = discovery
            .load_first()
            .expect("load figment")
            .expect("figment present");
        let config: SampleConfig = figment.extract().expect("extract sample config");
        assert!(!config.value);
        assert!(
            std::fs::metadata(&invalid).is_ok(),
            "expected invalid file retained"
        );
    }

    #[cfg(windows)]
    #[rstest]
    fn windows_candidates_are_case_insensitive() {
        use std::ffi::OsString;

        let _guards = clear_common_env();
        let mut builder = ConfigDiscovery::builder("hello_world");
        builder = builder.add_explicit_path(PathBuf::from("C:/Config/FILE.TOML"));
        builder = builder.add_explicit_path(PathBuf::from("c:/config/file.toml"));
        let discovery = builder.build();
        let candidates = discovery.candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].as_os_str(),
            OsString::from("C:/Config/FILE.TOML")
        );
    }
}
