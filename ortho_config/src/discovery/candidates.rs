//! Candidate-path generation and deduplication for `ConfigDiscovery`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dirs::home_dir;

use super::ConfigDiscovery;

#[cfg(windows)]
/// Normalises a path according to Windows' case-insensitive comparison rules by
/// lowercasing ASCII code points on the original wide path representation and
/// replacing forward slashes with backslashes.
fn windows_normalised_key(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;

    let normalised: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .map(|unit| match unit {
            65..=90 => unit + 32,
            47 => 92,
            _ => unit,
        })
        .collect();
    String::from_utf16_lossy(&normalised)
}

impl ConfigDiscovery {
    fn dedup_key(path: &Path) -> String {
        #[cfg(windows)]
        {
            windows_normalised_key(path)
        }

        #[cfg(not(windows))]
        {
            path.to_string_lossy().into_owned()
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
        let key = Self::dedup_key(&candidate);
        if seen.insert(key) {
            paths.push(candidate);
            true
        } else {
            false
        }
    }

    #[cfg(all(test, windows))]
    pub(super) fn normalised_key(path: &Path) -> String {
        Self::dedup_key(path)
    }

    fn candidates_for_base(&self, base_path: &Path) -> Vec<PathBuf> {
        let nested = if self.app_name.is_empty() {
            base_path.to_path_buf()
        } else {
            base_path.join(&self.app_name)
        };

        #[cfg(any(feature = "json5", feature = "yaml"))]
        let mut candidates = vec![
            nested.join(&self.config_file_name),
            base_path.join(&self.dotfile_name),
        ];
        #[cfg(not(any(feature = "json5", feature = "yaml")))]
        let candidates = vec![
            nested.join(&self.config_file_name),
            base_path.join(&self.dotfile_name),
        ];

        #[cfg(any(feature = "json5", feature = "yaml"))]
        if let Some(stem) = Path::new(&self.config_file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
        {
            #[cfg(feature = "json5")]
            Self::push_json_variant_candidates(&mut candidates, nested.as_path(), stem);
            #[cfg(feature = "yaml")]
            Self::push_yaml_variant_candidates(&mut candidates, nested.as_path(), stem);
        }

        candidates
    }

    fn push_for_bases<I>(&self, bases: I, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>)
    where
        I: IntoIterator,
        I::Item: Into<PathBuf>,
    {
        for base in bases {
            let base_path: PathBuf = base.into();
            for candidate in self.candidates_for_base(base_path.as_path()) {
                let _ = Self::push_unique(paths, seen, candidate);
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
        candidates: &mut Vec<PathBuf>,
        nested: &Path,
        stem: &str,
        extensions: &[&str],
    ) {
        for ext in extensions {
            let filename = format!("{stem}.{ext}");
            candidates.push(nested.join(&filename));
        }
    }

    #[cfg(feature = "json5")]
    fn push_json_variant_candidates(candidates: &mut Vec<PathBuf>, nested: &Path, stem: &str) {
        Self::push_variants_for_extensions(candidates, nested, stem, &["json", "json5"]);
    }

    #[cfg(feature = "yaml")]
    fn push_yaml_variant_candidates(candidates: &mut Vec<PathBuf>, nested: &Path, stem: &str) {
        Self::push_variants_for_extensions(candidates, nested, stem, &["yaml", "yml"]);
    }

    #[cfg(any(unix, target_os = "redox"))]
    fn push_default_xdg(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        self.push_for_bases(std::iter::once(PathBuf::from("/etc/xdg")), paths, seen);
    }

    #[cfg(not(any(unix, target_os = "redox")))]
    #[expect(
        clippy::unused_self,
        reason = "default XDG fallback does not apply on non-Unix/Redox targets"
    )]
    fn push_default_xdg(&self, _paths: &mut Vec<PathBuf>, _seen: &mut HashSet<String>) {}

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
