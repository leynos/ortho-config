//! Path discovery helpers for subcommand configuration.

use std::path::{Path, PathBuf};

use super::types::Prefix;

#[cfg(not(any(unix, target_os = "redox")))]
use directories::BaseDirs;
#[cfg(any(unix, target_os = "redox"))]
use xdg::BaseDirectories;

const EXT_GROUPS: &[&[&str]] = &[
    &["toml"],
    #[cfg(feature = "json5")]
    &["json", "json5"],
    #[cfg(feature = "yaml")]
    &["yaml", "yml"],
];

fn push_candidates<F>(paths: &mut Vec<PathBuf>, base: &str, mut to_path: F)
where
    F: FnMut(String) -> PathBuf,
{
    for group in EXT_GROUPS {
        for ext in *group {
            paths.push(to_path(format!("{base}.{ext}")));
        }
    }
}

fn dotted(prefix: &Prefix) -> String {
    let p = prefix.as_str();
    if p.is_empty() {
        String::new()
    } else {
        format!(".{p}")
    }
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

    for group in EXT_GROUPS {
        push_xdg_candidates(&xdg_dirs, group, paths);
    }
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
    #[cfg(any(unix, target_os = "redox"))]
    use super::*;
    #[cfg(any(unix, target_os = "redox"))]
    use rstest::rstest;
    #[cfg(any(unix, target_os = "redox"))]
    use serial_test::serial;
    #[cfg(any(unix, target_os = "redox"))]
    #[cfg(any(unix, target_os = "redox"))]
    use std::fs;
    #[cfg(any(unix, target_os = "redox"))]
    use std::path::Path;
    #[cfg(any(unix, target_os = "redox"))]
    use std::sync::LazyLock;
    #[cfg(any(unix, target_os = "redox"))]
    use tempfile::TempDir;

    #[cfg(any(unix, target_os = "redox"))]
    use test_helpers::env::{self as test_env, EnvVarGuard};

    #[cfg(any(unix, target_os = "redox"))]
    struct XdgGuard {
        dir: TempDir,
        _var: EnvVarGuard,
    }

    #[cfg(any(unix, target_os = "redox"))]
    static XDG_GUARD: LazyLock<XdgGuard> = LazyLock::new(|| {
        let dir = TempDir::new().expect("xdg");
        let path = dir.path().to_path_buf();
        let var = test_env::set_var("XDG_CONFIG_HOME", &path);
        XdgGuard { dir, _var: var }
    });

    #[cfg(any(unix, target_os = "redox"))]
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
        let dir = xdg_path();
        for entry in fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }

        for file in files {
            fs::write(dir.join(file), "").expect("create file");
        }

        let dirs = BaseDirectories::new();
        let mut paths = Vec::new();
        push_xdg_candidates(&dirs, exts, &mut paths);

        assert_eq!(paths.len(), files.len());
        for (p, f) in paths.iter().zip(files.iter()) {
            assert_eq!(p, &dir.join(f));
        }
    }

    #[cfg(any(unix, target_os = "redox"))]
    #[rstest]
    #[serial]
    #[case("")]
    #[case("myapp")]
    fn candidate_paths_ordering(#[case] prefix_raw: &str) {
        let home = TempDir::new().expect("home");
        let home_guard = test_env::set_var("HOME", home.path());

        let xdg_cfg_dir = if prefix_raw.is_empty() {
            xdg_path().to_path_buf()
        } else {
            let d = xdg_path().join(prefix_raw);
            fs::create_dir_all(&d).expect("xdg pref dir");
            d
        };

        fs::write(xdg_cfg_dir.join("config.toml"), "").expect("toml");
        #[cfg(feature = "yaml")]
        {
            fs::write(xdg_cfg_dir.join("config.yaml"), "").expect("yaml");
            fs::write(xdg_cfg_dir.join("config.yml"), "").expect("yml");
        }

        let prefix = Prefix::new(prefix_raw);
        let paths = candidate_paths(&prefix);

        let dotted_prefix = dotted(&prefix);

        let mut expected_files = Vec::new();
        for ext in EXT_GROUPS.iter().flat_map(|g| *g) {
            expected_files.push(format!("{dotted_prefix}.{ext}"));
        }
        expected_files.push("config.toml".to_string());
        #[cfg(feature = "yaml")]
        {
            expected_files.push("config.yaml".to_string());
            expected_files.push("config.yml".to_string());
        }
        for ext in EXT_GROUPS.iter().flat_map(|g| *g) {
            expected_files.push(format!("{dotted_prefix}.{ext}"));
        }

        let files: Vec<String> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, expected_files);

        let group_len: usize = EXT_GROUPS.iter().map(|g| g.len()).sum();

        let home_parent = paths[0].parent().unwrap();
        assert!(
            paths[..group_len]
                .iter()
                .all(|p| p.parent() == Some(home_parent))
        );

        let xdg_parent = paths[group_len].parent().unwrap();
        assert!(
            paths[group_len..paths.len() - group_len]
                .iter()
                .all(|p| p.parent() == Some(xdg_parent))
        );

        assert!(
            paths[paths.len() - group_len..]
                .iter()
                .all(|p| p.parent() == Some(Path::new(".")))
        );

        drop(home_guard);
    }
}
