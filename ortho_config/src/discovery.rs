//! Cross-platform configuration file discovery helpers.
//!
//! Applications can use [`ConfigDiscovery`] to enumerate configuration file
//! candidates in the same order exercised by the `hello_world` example. The
//! helper inspects explicit paths, XDG directories, Windows application data
//! folders, the user's home directory and project roots.
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::Utf8PathBuf;
use dirs::home_dir;

use crate::{OrthoError, OrthoResult, load_config_file};

mod builder;

pub use builder::ConfigDiscoveryBuilder;

/// Cross-platform configuration discovery helper mirroring the `hello_world` example.
#[derive(Debug, Clone)]
pub struct ConfigDiscovery {
    env_var: Option<String>,
    explicit_paths: Vec<PathBuf>,
    required_explicit_paths: Vec<PathBuf>,
    app_name: String,
    config_file_name: String,
    dotfile_name: String,
    project_file_name: String,
    project_roots: Vec<PathBuf>,
}

/// Result of a discovery attempt that keeps required and optional errors separate.
///
/// Callers can surface [`required_errors`] regardless of whether a configuration
/// file eventually loads, while deferring [`optional_errors`] until fallbacks are
/// exhausted. This mirrors the builder contract where required explicit paths
/// must exist.
///
/// # Examples
///
/// ```rust
/// use ortho_config::discovery::ConfigDiscovery;
///
/// let discovery = ConfigDiscovery::builder("demo")
///     .add_required_path("missing.toml")
///     .build();
/// let outcome = discovery.load_first_partitioned();
/// assert!(outcome.figment.is_none());
/// assert_eq!(outcome.required_errors.len(), 1);
/// ```
#[must_use]
pub struct DiscoveryLoadOutcome {
    /// Successfully loaded configuration file, if any.
    pub figment: Option<figment::Figment>,
    /// Errors originating from required explicit candidates.
    pub required_errors: Vec<Arc<OrthoError>>,
    /// Errors produced by optional discovery candidates.
    pub optional_errors: Vec<Arc<OrthoError>>,
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
        for path in &self.required_explicit_paths {
            Self::push_unique(paths, seen, path.clone());
        }

        for path in &self.explicit_paths {
            Self::push_unique(paths, seen, path.clone());
        }

        if let Some(value) = self
            .env_var
            .as_ref()
            .and_then(|env_var| std::env::var_os(env_var).filter(|v| !v.is_empty()))
        {
            Self::push_unique(paths, seen, PathBuf::from(value));
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
            #[cfg(any(feature = "json5", feature = "yaml"))]
            if let Some(stem) = Path::new(&self.config_file_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
            {
                #[cfg(feature = "json5")]
                {
                    for ext in ["json", "json5"] {
                        let filename = format!("{stem}.{ext}");
                        Self::push_unique(paths, seen, nested.join(&filename));
                    }
                }
                #[cfg(feature = "yaml")]
                {
                    for ext in ["yaml", "yml"] {
                        let filename = format!("{stem}.{ext}");
                        Self::push_unique(paths, seen, nested.join(&filename));
                    }
                }
            }
        }
    }

    fn push_xdg(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
            self.push_for_bases(std::iter::once(PathBuf::from(dir)), paths, seen);
        }

        if let Some(dirs) = std::env::var_os("XDG_CONFIG_DIRS") {
            let xdg_dirs: Vec<PathBuf> = std::env::split_paths(&dirs)
                .filter(|path| !path.as_os_str().is_empty())
                .collect();
            if xdg_dirs.is_empty() {
                if cfg!(any(unix, target_os = "redox")) {
                    self.push_for_bases(std::iter::once(PathBuf::from("/etc/xdg")), paths, seen);
                }
            } else {
                self.push_for_bases(xdg_dirs, paths, seen);
            }
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
    /// let discovery = ConfigDiscovery::builder("hello_world")
    ///     .add_explicit_path("./hello_world.toml")
    ///     .build();
    /// let mut utf8_candidates = discovery.utf8_candidates();
    /// assert_eq!(
    ///     utf8_candidates.remove(0),
    ///     camino::Utf8PathBuf::from("./hello_world.toml")
    /// );
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
        let (figment, errors) = self.load_first_with_errors();
        if let Some(figment) = figment {
            return Ok(Some(figment));
        }
        if let Some(err) = errors.into_iter().next() {
            return Err(err);
        }
        Ok(None)
    }

    /// Attempts to load the first available configuration file while partitioning errors.
    ///
    /// Required explicit candidates populate [`DiscoveryLoadOutcome::required_errors`]
    /// even when a later fallback succeeds, enabling callers to surface them eagerly.
    /// Optional candidates populate [`DiscoveryLoadOutcome::optional_errors`] so they
    /// can be reported once discovery exhausts every location.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::discovery::ConfigDiscovery;
    ///
    /// let discovery = ConfigDiscovery::builder("demo")
    ///     .add_required_path("missing.toml")
    ///     .build();
    /// let outcome = discovery.load_first_partitioned();
    /// assert!(outcome.figment.is_none());
    /// assert_eq!(outcome.required_errors.len(), 1);
    /// ```
    pub fn load_first_partitioned(&self) -> DiscoveryLoadOutcome {
        let mut required_errors = Vec::new();
        let mut optional_errors = Vec::new();
        let candidates = self.candidates();
        let required = self.required_explicit_paths.len();
        for (idx, path) in candidates.into_iter().enumerate() {
            match load_config_file(&path) {
                Ok(Some(figment)) => {
                    return DiscoveryLoadOutcome {
                        figment: Some(figment),
                        required_errors,
                        optional_errors,
                    };
                }
                Ok(None) => {
                    if idx < required {
                        let missing = Arc::new(OrthoError::File {
                            path: path.clone(),
                            source: Box::new(io::Error::new(
                                io::ErrorKind::NotFound,
                                "required configuration file not found",
                            )),
                        });
                        required_errors.push(missing);
                    }
                }
                Err(err) => {
                    if idx < required {
                        required_errors.push(err);
                    } else {
                        optional_errors.push(err);
                    }
                }
            }
        }
        DiscoveryLoadOutcome {
            figment: None,
            required_errors,
            optional_errors,
        }
    }

    /// Attempts to load the first available configuration file while collecting errors.
    #[must_use]
    pub fn load_first_with_errors(&self) -> (Option<figment::Figment>, Vec<Arc<OrthoError>>) {
        let DiscoveryLoadOutcome {
            figment,
            mut required_errors,
            mut optional_errors,
        } = self.load_first_partitioned();
        required_errors.append(&mut optional_errors);
        (figment, required_errors)
    }
}

#[cfg(test)]
mod tests;
