//! Candidate-path generation and deduplication for `ConfigDiscovery`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::ConfigDiscovery;
use super::telemetry;

#[cfg(windows)]
/// Normalizes a path according to Windows' case-insensitive comparison rules by
/// lowercasing ASCII code points on the original wide path representation and
/// replacing forward slashes with backslashes.
fn windows_normalized_key(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;

    let normalized: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .map(|unit| match unit {
            65..=90 => unit + 32,
            47 => 92,
            _ => unit,
        })
        .collect();
    String::from_utf16_lossy(&normalized)
}

impl ConfigDiscovery {
    fn dedup_key(path: &Path) -> String {
        #[cfg(windows)]
        {
            windows_normalized_key(path)
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
    pub(super) fn normalized_key(path: &Path) -> String {
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
        // An empty value must not contribute a base. `PathBuf::from("")` joined
        // with the app name yields a *relative* candidate such as
        // `demo/config.toml`, which would be resolved against the process's
        // working directory — loading configuration from wherever the tool
        // happens to be run. `XDG_CONFIG_DIRS` and the selector already guard
        // this; these three did not.
        let config_home = self.env_source.get("XDG_CONFIG_HOME");
        let config_home_state = telemetry::presence(config_home.as_ref());
        if let Some(dir) = config_home.filter(|value| !value.is_empty()) {
            self.push_for_bases(std::iter::once(PathBuf::from(dir)), paths, seen);
        }

        let dirs = self.env_source.get("XDG_CONFIG_DIRS");
        let dirs_state = telemetry::presence(dirs.as_ref());
        let resolution = self.push_xdg_dirs(dirs.as_ref(), paths, seen);

        telemetry::xdg_decision(config_home_state, dirs_state, resolution);
    }

    /// Push the `XDG_CONFIG_DIRS` bases, reporting which source supplied them.
    ///
    /// A list that is absent, or that contains only empty segments, falls back
    /// to the platform default. The two cases are reported separately because a
    /// value of `":"` is *present* yet still resolves to the default, and the
    /// distinction is exactly what makes a misconfigured list diagnosable.
    fn push_xdg_dirs(
        &self,
        dirs: Option<&std::ffi::OsString>,
        paths: &mut Vec<PathBuf>,
        seen: &mut HashSet<String>,
    ) -> &'static str {
        let Some(list) = dirs else {
            self.push_default_xdg(paths, seen);
            return telemetry::XDG_RESOLUTION_DEFAULT;
        };

        let mut xdg_dirs = std::env::split_paths(list)
            .filter(|path| !path.as_os_str().is_empty())
            .peekable();
        if xdg_dirs.peek().is_none() {
            self.push_default_xdg(paths, seen);
            return telemetry::XDG_RESOLUTION_DEFAULT;
        }

        self.push_for_bases(xdg_dirs, paths, seen);
        telemetry::XDG_RESOLUTION_LIST
    }

    fn push_windows(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        let dirs = ["APPDATA", "LOCALAPPDATA"].into_iter().filter_map(|key| {
            self.env_source
                .get(key)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        });
        self.push_for_bases(dirs, paths, seen);
    }

    /// Read `key`, treating an empty value as unset.
    ///
    /// Every environment-derived base directory goes through this: an empty
    /// value joined with an application name produces a working-directory
    /// relative candidate, which would load configuration from wherever the
    /// tool happened to be run.
    fn non_empty(&self, key: &str) -> Option<std::ffi::OsString> {
        self.env_source.get(key).filter(|value| !value.is_empty())
    }

    /// Resolve the home directory, reporting which source named it.
    ///
    /// `HOME` outranks `USERPROFILE`, and the source's own platform fallback is
    /// consulted only when neither is set — an injected source returns `None`
    /// there, which is what keeps a test's candidate list independent of the
    /// host machine.
    ///
    /// An empty value is treated as unset, for the same reason the XDG and
    /// Windows base directories are: `PathBuf::from("")` joined with `.config`
    /// yields a *relative* path, so an empty `HOME` would silently search the
    /// process's working directory. Treating it as unset also lets a populated
    /// `USERPROFILE` be reached, which an empty `HOME` would otherwise block.
    fn resolve_home(&self) -> (Option<PathBuf>, &'static str) {
        if let Some(value) = self.non_empty("HOME") {
            return (Some(PathBuf::from(value)), telemetry::HOME_FROM_HOME);
        }
        if let Some(value) = self.non_empty("USERPROFILE") {
            return (Some(PathBuf::from(value)), telemetry::HOME_FROM_USERPROFILE);
        }
        self.env_source
            .home_fallback()
            .map_or((None, telemetry::HOME_NONE), |path| {
                (Some(path), telemetry::HOME_FROM_FALLBACK)
            })
    }

    fn push_home(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        let (home, source) = self.resolve_home();
        telemetry::home_decision(source);
        if let Some(home_path) = home {
            let config_dir = home_path.join(".config");
            self.push_for_bases(std::iter::once(config_dir), paths, seen);
            Self::push_unique(paths, seen, home_path.join(&self.dotfile_name));
        }
    }

    /// Push the configuration-path selector, reporting how it resolved.
    ///
    /// The three non-accepting states are kept distinct because they call for
    /// different action: no selector was configured at all, one was configured
    /// but the operator has not set it, or it is set to an empty value — the
    /// last being a likely mistake that discovery deliberately ignores.
    fn push_selector(&self, paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
        let Some(env_var) = self.env_var.as_ref() else {
            telemetry::selector_decision(telemetry::SELECTOR_NOT_CONFIGURED);
            return;
        };

        match self.env_source.get(env_var) {
            None => telemetry::selector_decision(telemetry::SELECTOR_UNSET),
            Some(value) if value.is_empty() => {
                telemetry::selector_decision(telemetry::SELECTOR_EMPTY);
            }
            Some(value) => {
                telemetry::selector_decision(telemetry::SELECTOR_ACCEPTED);
                let _ = Self::push_unique(paths, seen, PathBuf::from(value));
            }
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
        clippy::missing_const_for_fn,
        reason = "signature must match the Unix variant which is not const"
    )]
    #[expect(
        clippy::ptr_arg,
        reason = "signature must match the Unix variant which requires Vec for push"
    )]
    fn push_default_xdg(&self, _paths: &mut Vec<PathBuf>, _seen: &mut HashSet<String>) {
        _ = self;
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

        self.push_selector(&mut paths, &mut seen);
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
