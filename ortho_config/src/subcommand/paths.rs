//! Path discovery helpers for subcommand configuration.

use std::path::{Path, PathBuf};

use super::types::Prefix;

#[cfg(not(any(unix, target_os = "redox")))]
use directories::BaseDirs;
#[cfg(any(unix, target_os = "redox"))]
use xdg::BaseDirectories;

fn push_candidates<F>(paths: &mut Vec<PathBuf>, base: &str, mut to_path: F)
where
    F: FnMut(String) -> PathBuf,
{
    paths.push(to_path(format!("{base}.toml")));
    #[cfg(feature = "json5")]
    for ext in ["json", "json5"] {
        paths.push(to_path(format!("{base}.{ext}")));
    }
    #[cfg(feature = "yaml")]
    for ext in ["yaml", "yml"] {
        paths.push(to_path(format!("{base}.{ext}")));
    }
}

fn dotted(prefix: &Prefix) -> String {
    format!(".{}", prefix.as_str())
}

/// Adds candidate configuration file paths under `dir` using `base` as the file stem.
///
/// The `base` string should include any desired prefix such as a leading dot.
/// Supported configuration extensions are appended and each candidate is joined
/// with `dir` before being pushed onto `paths`.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::{Path, PathBuf};
/// use ortho_config::subcommand::push_stem_candidates;
/// let mut candidates: Vec<PathBuf> = Vec::new();
/// // Populate the vector with common configuration file names under `/tmp`.
/// push_stem_candidates(Path::new("/tmp"), ".myapp", &mut candidates);
/// assert!(candidates.iter().any(|p| p.ends_with(".myapp.toml")));
/// ```
pub fn push_stem_candidates(dir: &Path, base: &str, paths: &mut Vec<PathBuf>) {
    push_candidates(paths, base, |f| dir.join(f));
}

fn push_local_candidates(prefix: &Prefix, paths: &mut Vec<PathBuf>) {
    push_stem_candidates(Path::new("."), &dotted(prefix), paths);
}

/// Adds XDG configuration files for the provided extensions.
///
/// Iterates over `exts`, searching `xdg_dirs` for `config.<ext>` and pushes each
/// discovered path onto `paths`.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::PathBuf;
/// use xdg::BaseDirectories;
/// use ortho_config::subcommand::paths::push_xdg_candidates;
/// let dirs = BaseDirectories::new().expect("locate directories");
/// let mut paths: Vec<PathBuf> = Vec::new();
/// push_xdg_candidates(&dirs, &["toml"], &mut paths);
/// assert!(paths.iter().all(|p| p.ends_with("config.toml")));
/// ```
#[cfg(any(unix, target_os = "redox"))]
fn push_xdg_candidates(xdg_dirs: &BaseDirectories, exts: &[&str], paths: &mut Vec<PathBuf>) {
    for ext in exts {
        if let Some(p) = xdg_dirs.find_config_file(format!("config.{ext}")) {
            paths.push(p);
        }
    }
}

#[cfg(any(unix, target_os = "redox"))]
pub(crate) fn collect_unix_paths(prefix: &Prefix, paths: &mut Vec<PathBuf>) {
    let dotted = dotted(prefix);
    if let Some(home) = std::env::var_os("HOME") {
        push_stem_candidates(Path::new(&home), &dotted, paths);
    }

    let xdg_dirs = if prefix.as_str().is_empty() {
        BaseDirectories::new()
    } else {
        BaseDirectories::with_prefix(prefix.as_str())
    };

    push_xdg_candidates(&xdg_dirs, &["toml"], paths);

    #[cfg(feature = "json5")]
    push_xdg_candidates(&xdg_dirs, &["json", "json5"], paths);

    #[cfg(feature = "yaml")]
    push_xdg_candidates(&xdg_dirs, &["yaml", "yml"], paths);
}

#[cfg(not(any(unix, target_os = "redox")))]
pub(crate) fn collect_non_unix_paths(prefix: &Prefix, paths: &mut Vec<PathBuf>) {
    let dotted = dotted(prefix);

    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        push_stem_candidates(Path::new(&home), &dotted, paths);
    }

    if let Some(dirs) = BaseDirs::new() {
        if std::env::var_os("HOME").is_none() && std::env::var_os("USERPROFILE").is_none() {
            push_stem_candidates(dirs.home_dir(), &dotted, paths);
        }

        let cfg_dir = if prefix.as_str().is_empty() {
            dirs.config_dir().to_path_buf()
        } else {
            dirs.config_dir().join(prefix.as_str())
        };
        push_stem_candidates(&cfg_dir, "config", paths);
    }
}

pub(crate) fn candidate_paths(prefix: &Prefix) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(any(unix, target_os = "redox"))]
    collect_unix_paths(prefix, &mut paths);

    #[cfg(not(any(unix, target_os = "redox")))]
    collect_non_unix_paths(prefix, &mut paths);

    push_local_candidates(prefix, &mut paths);
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serial_test::serial;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;
    use std::sync::LazyLock;
    use tempfile::TempDir;

    struct XdgGuard {
        old: Option<OsString>,
        dir: TempDir,
    }

    impl Drop for XdgGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
                None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
            }
        }
    }

    static XDG_GUARD: LazyLock<XdgGuard> = LazyLock::new(|| {
        let old = env::var_os("XDG_CONFIG_HOME");
        let dir = TempDir::new().expect("xdg");
        let path = dir.path().to_path_buf();
        unsafe {
            env::set_var("XDG_CONFIG_HOME", &path);
        }
        XdgGuard { old, dir }
    });

    fn xdg_path() -> &'static Path {
        XDG_GUARD.dir.path()
    }

    #[cfg(any(unix, target_os = "redox"))]
    #[rstest]
    #[serial]
    #[case(&["toml"], &["config.toml"])]
    #[cfg(feature = "json5")]
    #[case(&["json", "json5"], &["config.json", "config.json5"])]
    #[cfg(feature = "yaml")]
    #[case(&["yaml", "yml"], &["config.yaml", "config.yml"])]
    fn push_xdg_candidates_finds_files(#[case] exts: &[&str], #[case] files: &[&str]) {
        let dir = TempDir::new().expect("tempdir");
        let old = env::var_os("XDG_CONFIG_HOME");
        unsafe {
            env::set_var("XDG_CONFIG_HOME", dir.path());
        }

        for file in files {
            fs::write(dir.path().join(file), "").expect("create file");
        }

        let dirs = BaseDirectories::new();
        let mut paths = Vec::new();
        push_xdg_candidates(&dirs, exts, &mut paths);

        assert_eq!(paths.len(), files.len());
        for (p, f) in paths.iter().zip(files.iter()) {
            assert_eq!(p, &dir.path().join(f));
        }

        if let Some(v) = old {
            unsafe {
                env::set_var("XDG_CONFIG_HOME", v);
            }
        } else {
            unsafe {
                env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }

    #[cfg(any(unix, target_os = "redox"))]
    #[rstest]
    #[serial]
    #[case("")]
    #[case("myapp")]
    fn candidate_paths_ordering(#[case] prefix_raw: &str) {
        let home = TempDir::new().expect("home");
        let old_home = env::var_os("HOME");
        unsafe {
            env::set_var("HOME", home.path());
        }

        let xdg_cfg_dir = if prefix_raw.is_empty() {
            xdg_path().to_path_buf()
        } else {
            let d = xdg_path().join(prefix_raw);
            fs::create_dir_all(&d).expect("xdg pref dir");
            d
        };

        fs::write(xdg_cfg_dir.join("config.toml"), "").expect("toml");
        #[cfg(feature = "json5")]
        {
            fs::write(xdg_cfg_dir.join("config.json"), "").expect("json");
            fs::write(xdg_cfg_dir.join("config.json5"), "").expect("json5");
        }
        #[cfg(feature = "yaml")]
        {
            fs::write(xdg_cfg_dir.join("config.yaml"), "").expect("yaml");
            fs::write(xdg_cfg_dir.join("config.yml"), "").expect("yml");
        }

        let prefix = Prefix::new(prefix_raw);
        let paths = candidate_paths(&prefix);

        let dotted = if prefix.as_str().is_empty() {
            ".".to_string()
        } else {
            format!(".{}", prefix.as_str())
        };

        let mut expected = Vec::new();
        expected.push(home.path().join(format!("{dotted}.toml")));
        #[cfg(feature = "json5")]
        {
            expected.push(home.path().join(format!("{dotted}.json")));
            expected.push(home.path().join(format!("{dotted}.json5")));
        }
        #[cfg(feature = "yaml")]
        {
            expected.push(home.path().join(format!("{dotted}.yaml")));
            expected.push(home.path().join(format!("{dotted}.yml")));
        }
        expected.push(xdg_cfg_dir.join("config.toml"));
        #[cfg(feature = "json5")]
        {
            expected.push(xdg_cfg_dir.join("config.json"));
            expected.push(xdg_cfg_dir.join("config.json5"));
        }
        #[cfg(feature = "yaml")]
        {
            expected.push(xdg_cfg_dir.join("config.yaml"));
            expected.push(xdg_cfg_dir.join("config.yml"));
        }
        expected.push(Path::new(".").join(format!("{dotted}.toml")));
        #[cfg(feature = "json5")]
        {
            expected.push(Path::new(".").join(format!("{dotted}.json")));
            expected.push(Path::new(".").join(format!("{dotted}.json5")));
        }
        #[cfg(feature = "yaml")]
        {
            expected.push(Path::new(".").join(format!("{dotted}.yaml")));
            expected.push(Path::new(".").join(format!("{dotted}.yml")));
        }

        assert_eq!(paths, expected);

        if let Some(v) = old_home {
            unsafe {
                env::set_var("HOME", v);
            }
        } else {
            unsafe {
                env::remove_var("HOME");
            }
        }
    }
}
