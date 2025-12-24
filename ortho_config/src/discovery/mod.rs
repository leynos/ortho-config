//! Cross-platform configuration file discovery helpers.
//!
//! Applications can use [`ConfigDiscovery`] to enumerate configuration file
//! candidates in the same order exercised by the `hello_world` example. The
//! helper inspects explicit paths, XDG directories, Windows application data
//! folders, the user's home directory and project roots.
use std::borrow::Cow;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::Utf8PathBuf;
use dirs::home_dir;

use crate::{
    FileLayerChain, MergeLayer, OrthoError, OrthoMergeExt, OrthoResult, load_config_file,
    load_config_file_as_chain,
};

#[cfg(windows)]
/// Normalises a path according to Windows' case-insensitive comparison rules by
/// lowercasing Unicode scalar values and replacing forward slashes with
/// backslashes, mirroring the filesystem's treatment of separators.
fn windows_normalised_key(path: &Path) -> String {
    let mut lowercased = path.to_string_lossy().to_lowercase();
    if lowercased.contains('/') {
        lowercased = lowercased.replace('/', "\\");
    }
    lowercased
}

mod builder;
mod outcome;

pub use builder::ConfigDiscoveryBuilder;
use outcome::DiscoveryOutcome;

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
/// Callers can surface [`DiscoveryLoadOutcome::required_errors`] regardless of whether a configuration
/// file eventually loads, while deferring [`DiscoveryLoadOutcome::optional_errors`] until fallbacks are
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
#[derive(Debug, Default)]
#[must_use]
pub struct DiscoveryLoadOutcome {
    /// Successfully loaded configuration file, if any.
    pub figment: Option<figment::Figment>,
    /// Errors originating from required explicit candidates.
    pub required_errors: Vec<Arc<OrthoError>>,
    /// Errors produced by optional discovery candidates.
    pub optional_errors: Vec<Arc<OrthoError>>,
}

/// Composition result that captures the first discovered configuration layer.
#[derive(Debug, Default)]
#[must_use]
pub struct DiscoveryLayerOutcome {
    /// Successfully composed merge layer, if any.
    pub layer: Option<MergeLayer<'static>>,
    /// Errors originating from required explicit candidates.
    pub required_errors: Vec<Arc<OrthoError>>,
    /// Errors produced by optional discovery candidates.
    pub optional_errors: Vec<Arc<OrthoError>>,
}

/// Composition result that captures multiple file layers from an extends chain.
///
/// When a configuration file uses `extends`, each file in the inheritance chain
/// is returned as a separate layer. This allows declarative merge strategies
/// (such as append for vectors) to be applied across the chain.
#[derive(Debug, Default)]
#[must_use]
pub struct DiscoveryLayersOutcome {
    /// Successfully composed merge layers from the file chain.
    /// Ordered ancestor-first when extends is used.
    pub layers: Vec<MergeLayer<'static>>,
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

    fn discover_first<T, F>(&self, mut build: F) -> DiscoveryOutcome<T>
    where
        F: FnMut(figment::Figment, &Path) -> Result<T, Arc<OrthoError>>,
    {
        let mut required_errors = Vec::new();
        let mut optional_errors = Vec::new();
        let (candidates, required_bound) = self.candidates_with_required_bound();
        for (idx, path) in candidates.into_iter().enumerate() {
            match load_config_file(&path) {
                Ok(Some(figment)) => match build(figment, &path) {
                    Ok(value) => {
                        return DiscoveryOutcome {
                            value: Some(value),
                            required_errors,
                            optional_errors,
                        };
                    }
                    Err(err) if idx < required_bound => required_errors.push(err),
                    Err(err) => optional_errors.push(err),
                },
                Ok(None) if idx < required_bound => {
                    required_errors.push(Self::missing_required_error(&path));
                }
                Ok(None) => {}
                Err(err) if idx < required_bound => required_errors.push(err),
                Err(err) => optional_errors.push(err),
            }
        }
        DiscoveryOutcome {
            value: None,
            required_errors,
            optional_errors,
        }
    }

    fn discover_first_chain<T, F>(&self, mut build: F) -> DiscoveryOutcome<T>
    where
        F: FnMut(FileLayerChain) -> Result<T, Arc<OrthoError>>,
    {
        let mut required_errors = Vec::new();
        let mut optional_errors = Vec::new();
        let (candidates, required_bound) = self.candidates_with_required_bound();
        for (idx, path) in candidates.into_iter().enumerate() {
            match load_config_file_as_chain(&path) {
                Ok(Some(chain)) => match build(chain) {
                    Ok(value) => {
                        return DiscoveryOutcome {
                            value: Some(value),
                            required_errors,
                            optional_errors,
                        };
                    }
                    Err(err) if idx < required_bound => required_errors.push(err),
                    Err(err) => optional_errors.push(err),
                },
                Ok(None) if idx < required_bound => {
                    required_errors.push(Self::missing_required_error(&path));
                }
                Ok(None) => {}
                Err(err) if idx < required_bound => required_errors.push(err),
                Err(err) => optional_errors.push(err),
            }
        }
        DiscoveryOutcome {
            value: None,
            required_errors,
            optional_errors,
        }
    }

    fn push_unique(
        paths: &mut Vec<PathBuf>,
        seen: &mut HashSet<String>,
        candidate: PathBuf,
    ) -> bool {
        if candidate.as_os_str().is_empty() {
            return false;
        }
        let key = Self::normalised_key(&candidate);
        if seen.insert(key) {
            paths.push(candidate);
            true
        } else {
            false
        }
    }

    fn normalised_key(path: &Path) -> String {
        #[cfg(windows)]
        {
            windows_normalised_key(path)
        }

        #[cfg(not(windows))]
        {
            path.to_string_lossy().into_owned()
        }
    }

    fn push_for_bases<I>(&self, bases: I, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>)
    where
        I: IntoIterator,
        I::Item: Into<PathBuf>,
    {
        for base in bases {
            let base_path: PathBuf = base.into();
            let nested = if self.app_name.is_empty() {
                base_path.clone()
            } else {
                base_path.join(&self.app_name)
            };
            Self::push_unique(paths, seen, nested.join(&self.config_file_name));
            Self::push_unique(paths, seen, base_path.join(&self.dotfile_name));
            #[cfg(any(feature = "json5", feature = "yaml"))]
            if let Some(stem) = Path::new(&self.config_file_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
            {
                #[cfg(feature = "json5")]
                Self::push_json_variants(paths, seen, nested.as_path(), stem);
                #[cfg(feature = "yaml")]
                Self::push_yaml_variants(paths, seen, nested.as_path(), stem);
            }
        }
    }

    fn push_xdg(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
            self.push_for_bases(std::iter::once(PathBuf::from(dir)), paths, seen);
        }

        match std::env::var_os("XDG_CONFIG_DIRS") {
            Some(dirs) => {
                let mut xdg_dirs = std::env::split_paths(&dirs)
                    .filter(|path| !path.as_os_str().is_empty())
                    .peekable();
                if xdg_dirs.peek().is_none() {
                    self.push_default_xdg(paths, seen);
                } else {
                    self.push_for_bases(xdg_dirs, paths, seen);
                }
            }
            None => self.push_default_xdg(paths, seen),
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

    #[cfg(any(feature = "json5", feature = "yaml"))]
    fn push_variants_for_extensions(
        (paths, seen): (&mut Vec<PathBuf>, &mut HashSet<String>),
        nested: &Path,
        stem: &str,
        extensions: &[&str],
    ) {
        for ext in extensions {
            let filename = format!("{stem}.{ext}");
            Self::push_unique(paths, seen, nested.join(&filename));
        }
    }

    #[cfg(feature = "json5")]
    fn push_json_variants(
        paths: &mut Vec<PathBuf>,
        seen: &mut HashSet<String>,
        nested: &Path,
        stem: &str,
    ) {
        Self::push_variants_for_extensions((paths, seen), nested, stem, &["json", "json5"]);
    }

    #[cfg(feature = "yaml")]
    fn push_yaml_variants(
        paths: &mut Vec<PathBuf>,
        seen: &mut HashSet<String>,
        nested: &Path,
        stem: &str,
    ) {
        Self::push_variants_for_extensions((paths, seen), nested, stem, &["yaml", "yml"]);
    }

    #[cfg_attr(
        not(any(unix, target_os = "redox")),
        expect(
            clippy::unused_self,
            reason = "self is used on Unix/Redox platforms via push_for_bases"
        )
    )]
    #[cfg_attr(
        any(unix, target_os = "redox"),
        expect(
            clippy::used_underscore_binding,
            reason = "underscore-prefixed parameters avoid unused warnings on other platforms"
        )
    )]
    #[cfg_attr(
        windows,
        expect(
            clippy::missing_const_for_fn,
            reason = "Windows builds do not call `push_for_bases`, but Unix builds rely on runtime allocation"
        )
    )]
    fn push_default_xdg(&self, _paths: &mut Vec<PathBuf>, _seen: &mut HashSet<String>) {
        #[cfg(any(unix, target_os = "redox"))]
        self.push_for_bases(std::iter::once(PathBuf::from("/etc/xdg")), _paths, _seen);
    }

    /// Returns the ordered configuration candidates.
    #[must_use]
    pub fn candidates(&self) -> Vec<PathBuf> {
        self.candidates_with_required_bound().0
    }

    fn candidates_with_required_bound(&self) -> (Vec<PathBuf>, usize) {
        let mut seen: HashSet<String> = HashSet::new();
        let mut paths = Vec::new();
        let mut required_bound = 0;

        for path in &self.required_explicit_paths {
            if Self::push_unique(&mut paths, &mut seen, path.clone()) {
                required_bound += 1;
            }
        }

        for path in &self.explicit_paths {
            let _ = Self::push_unique(&mut paths, &mut seen, path.clone());
        }

        if let Some(value) = self
            .env_var
            .as_ref()
            .and_then(|env_var| std::env::var_os(env_var).filter(|v| !v.is_empty()))
        {
            let _ = Self::push_unique(&mut paths, &mut seen, PathBuf::from(value));
        }

        self.push_xdg(&mut paths, &mut seen);
        self.push_windows(&mut paths, &mut seen);
        self.push_home(&mut paths, &mut seen);
        self.push_projects(&mut paths, &mut seen);

        (paths, required_bound)
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
    /// When every candidate fails, returns an error containing all recorded
    /// discovery diagnostics; if no candidates exist, returns `Ok(None)`.
    pub fn load_first(&self) -> OrthoResult<Option<figment::Figment>> {
        let (figment, errors) = self.load_first_with_errors();
        if let Some(found_figment) = figment {
            return Ok(Some(found_figment));
        }
        if let Some(err) = OrthoError::try_aggregate(errors) {
            return Err(Arc::new(err));
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
        let outcome = self.discover_first(|figment, _| Ok(figment));
        DiscoveryLoadOutcome {
            figment: outcome.value,
            required_errors: outcome.required_errors,
            optional_errors: outcome.optional_errors,
        }
    }

    /// Composes the first available configuration file into a merge layer.
    ///
    /// Captures errors for required and optional candidates separately so
    /// callers can mirror the aggregation semantics of [`Self::load_first`].
    pub fn compose_layer(&self) -> DiscoveryLayerOutcome {
        let outcome = self.discover_first(|figment, path| {
            figment
                .extract::<crate::serde_json::Value>()
                .into_ortho_merge()
                .map(|value| {
                    let utf8_path = Utf8PathBuf::from_path_buf(path.to_path_buf())
                        .ok()
                        .unwrap_or_else(|| Utf8PathBuf::from(path.to_string_lossy().into_owned()));
                    MergeLayer::file(Cow::Owned(value), Some(utf8_path))
                })
        });
        DiscoveryLayerOutcome {
            layer: outcome.value,
            required_errors: outcome.required_errors,
            optional_errors: outcome.optional_errors,
        }
    }

    /// Composes the first available configuration file into multiple merge layers.
    ///
    /// Unlike [`compose_layer`](Self::compose_layer), this method preserves each
    /// file in an `extends` chain as a separate layer. This allows declarative
    /// merge strategies (such as append for vectors) to be applied across the
    /// inheritance chain rather than using Figment's replacement semantics.
    ///
    /// Captures errors for required and optional candidates separately so
    /// callers can mirror the aggregation semantics of [`Self::load_first`].
    pub fn compose_layers(&self) -> DiscoveryLayersOutcome {
        let outcome = self.discover_first_chain(|chain| {
            let layers = chain
                .values
                .into_iter()
                .map(|(value, path)| MergeLayer::file(Cow::Owned(value), Some(path)))
                .collect();
            Ok(layers)
        });
        DiscoveryLayersOutcome {
            layers: outcome.value.unwrap_or_default(),
            required_errors: outcome.required_errors,
            optional_errors: outcome.optional_errors,
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

    fn missing_required_error(path: &Path) -> Arc<OrthoError> {
        Arc::new(OrthoError::File {
            path: path.to_path_buf(),
            source: Box::new(io::Error::new(
                io::ErrorKind::NotFound,
                "required configuration file not found",
            )),
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod dedup_tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[cfg(windows)]
    fn canonicalish(path: &Path) -> PathBuf {
        match dunce::canonicalize(path) {
            Ok(p) => p,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
            Err(err) => panic!("failed to canonicalise {path:?}: {err}"),
        }
    }

    #[cfg(not(windows))]
    fn canonicalish(path: &Path) -> PathBuf {
        path.to_path_buf()
    }

    fn assert_first_error_path(errors: &[Arc<OrthoError>], expected: &Path) {
        let err = errors
            .first()
            .expect("expected at least one error when asserting path");
        let path = match err.as_ref() {
            OrthoError::File { path, .. } => path,
            other => panic!("expected OrthoError::File, got {other:?}"),
        };
        assert_eq!(canonicalish(path), canonicalish(expected));
    }

    #[test]
    fn load_first_partitioned_dedups_required_paths() {
        let dir = tempdir().expect("create tempdir");
        let required = dir.path().join("missing.toml");
        let optional = dir.path().join("optional.toml");
        fs::write(&optional, "invalid = {").expect("write invalid optional config");
        let discovery = ConfigDiscovery::builder("app")
            .add_required_path(&required)
            .add_required_path(&required)
            .add_explicit_path(&optional)
            .build();

        let outcome = discovery.load_first_partitioned();
        assert!(outcome.figment.is_none());
        assert_eq!(outcome.required_errors.len(), 1);
        assert_eq!(outcome.optional_errors.len(), 1);

        assert_first_error_path(&outcome.required_errors, &required);
        assert_first_error_path(&outcome.optional_errors, &optional);
    }

    #[cfg(windows)]
    #[test]
    fn normalised_key_lowercases_ascii_and_backslashes() {
        let key = ConfigDiscovery::normalised_key(Path::new("C:/Config/FILE.TOML"));
        assert_eq!(key, "c:\\config\\file.toml");
    }

    #[cfg(windows)]
    #[test]
    fn normalised_key_handles_unicode_case() {
        let key = ConfigDiscovery::normalised_key(Path::new("C:/Temp/CAFÉ.toml"));
        assert_eq!(key, "c:\\temp\\café.toml");
    }
}
