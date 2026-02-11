//! Candidate-path generation and deduplication for `ConfigDiscovery`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dirs::home_dir;

use super::ConfigDiscovery;

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

impl ConfigDiscovery {
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

    pub(super) fn normalised_key(path: &Path) -> String {
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
    #[cfg_attr(
        windows,
        expect(
            clippy::ptr_arg,
            reason = "Windows builds do not use paths, but Unix builds push via `push_for_bases`"
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

    pub(super) fn candidates_with_required_bound(&self) -> (Vec<PathBuf>, usize) {
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

    /// Returns the ordered configuration candidates as [`camino::Utf8PathBuf`] values.
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
    pub fn utf8_candidates(&self) -> Vec<camino::Utf8PathBuf> {
        self.candidates()
            .into_iter()
            .filter_map(|path| camino::Utf8PathBuf::from_path_buf(path).ok())
            .collect()
    }
}
