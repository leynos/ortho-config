//! Utilities for discovering configuration file paths for subcommands.
//!
//! Enumerates candidate configuration files under the user's home directory,
//! platform-specific configuration directories (e.g. XDG locations), and the
//! current working directory.

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
/// use ortho_config::subcommand::paths::push_stem_candidates;
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
/// let dirs = BaseDirectories::new();
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

    // Only search for canonical XDG config filenames under the XDG dirs:
    // - config.toml (always)
    // - config.yaml and config.yml when the `yaml` feature is enabled
    // - config.json and config.json5 when the `json5` feature is enabled
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

/// Returns candidate configuration file paths for `prefix`.
///
/// Paths are yielded in the following order:
/// 1. The user's home directory, e.g. `~/.app.toml`.
/// 2. Platform configuration directories such as
///    `$XDG_CONFIG_HOME/app/config.toml`.
/// 3. The current working directory, e.g. `./.app.toml`.
///
/// The [`Prefix`] normalises user input and is incorporated into file stems and
/// directory names. When `prefix` is empty, home and working directories yield
/// dotfiles with only an extension (e.g. `~/.toml`, `./.toml`). Platform
/// configuration directories are searched solely for their canonical
/// `config.<ext>` names as defined by the platform (e.g. `config.toml` under
/// `$XDG_CONFIG_HOME`).
#[cfg_attr(
    feature = "json5",
    doc = "On Unix-like platforms these may also be `config.json` and `config.json5`."
)]
/// This restriction applies only to platform directories; home and working
/// directories still emit extension-only dotfiles.
///
/// # Examples
///
/// ```rust,ignore
/// use ortho_config::subcommand::{paths::candidate_paths, Prefix};
///
/// let paths = candidate_paths(&Prefix::new("app"));
/// // prints something like:
/// // ["/home/alice/.app.toml",
/// //  "/home/alice/.config/app/config.toml",
/// //  "./.app.toml"]
/// println!("{paths:?}");
/// ```
///
/// ```rust,ignore
/// // Empty prefix: home/local dotfiles with no stem plus platform config.* files
/// let paths = candidate_paths(&Prefix::new(""));
/// // e.g. ["/home/alice/.toml", "/home/alice/.config/config.toml", "./.toml"]
/// println!("{paths:?}");
/// ```
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
    use anyhow::{Context, Result, anyhow, ensure};
    #[cfg(any(unix, target_os = "redox"))]
    use rstest::rstest;
    #[cfg(any(unix, target_os = "redox"))]
    use std::fs;
    #[cfg(any(unix, target_os = "redox"))]
    use std::path::Path;
    #[cfg(any(unix, target_os = "redox"))]
    use tempfile::TempDir;

    #[cfg(any(unix, target_os = "redox"))]
    use test_helpers::env::{self as test_env, EnvVarGuard};

    #[cfg(any(unix, target_os = "redox"))]
    /// Creates a temporary XDG config directory and sets `XDG_CONFIG_HOME` for the test.
    fn init_xdg_home() -> Result<(TempDir, EnvVarGuard)> {
        let dir = TempDir::new().context("create XDG config temp directory")?;
        let guard = test_env::set_var("XDG_CONFIG_HOME", dir.path());
        Ok((dir, guard))
    }

    #[cfg(any(unix, target_os = "redox"))]
    #[rstest]
    #[case(&["toml"], &["config.toml"])]
    #[cfg(feature = "json5")]
    #[case(&["json", "json5"], &["config.json", "config.json5"])]
    #[cfg(feature = "yaml")]
    #[case(&["yaml", "yml"], &["config.yaml", "config.yml"])]
    fn push_xdg_candidates_finds_files(
        #[case] exts: &[&str],
        #[case] files: &[&str],
    ) -> Result<()> {
        let (xdg_tempdir, _guard) = init_xdg_home()?;
        let dir_path = xdg_tempdir.path();
        // TempDir is empty on creation; no cleanup required.

        for file in files {
            fs::write(dir_path.join(file), "")
                .with_context(|| format!("create test file {file}"))?;
        }

        let dirs = BaseDirectories::new();
        let mut paths = Vec::new();
        push_xdg_candidates(&dirs, exts, &mut paths);

        ensure!(
            paths.len() == files.len(),
            "expected {:?} to locate {} files, found {}",
            exts,
            files.len(),
            paths.len()
        );
        for (p, f) in paths.iter().zip(files.iter()) {
            ensure!(
                p == &dir_path.join(f),
                "unexpected path {p:?} for expected file {f}"
            );
        }
        Ok(())
    }

    #[cfg(any(unix, target_os = "redox"))]
    #[rstest]
    #[case("")]
    #[case("myapp")]
    fn candidate_paths_ordering(#[case] prefix_raw: &str) -> Result<()> {
        let home = TempDir::new().context("create home directory")?;
        let home_guard = test_env::set_var("HOME", home.path());

        let (base_dir, _guard) = init_xdg_home()?;
        let base = base_dir.path();
        let xdg_cfg_dir = if prefix_raw.is_empty() {
            base.to_path_buf()
        } else {
            let d = base.join(prefix_raw);
            fs::create_dir_all(&d).context("create prefixed XDG directory")?;
            d
        };

        fs::write(xdg_cfg_dir.join("config.toml"), "").context("write config.toml")?;
        #[cfg(feature = "yaml")]
        {
            fs::write(xdg_cfg_dir.join("config.yaml"), "").context("write config.yaml")?;
            fs::write(xdg_cfg_dir.join("config.yml"), "").context("write config.yml")?;
        }

        let prefix = Prefix::new(prefix_raw);
        let paths = candidate_paths(&prefix);

        let dotted_prefix = dotted(&prefix);

        let mut expected_files = Vec::new();
        for ext in EXT_GROUPS.iter().flat_map(|g| *g) {
            expected_files.push(format!("{dotted_prefix}.{ext}"));
        }
        expected_files.push("config.toml".to_owned());
        #[cfg(feature = "yaml")]
        {
            expected_files.push("config.yaml".to_owned());
            expected_files.push("config.yml".to_owned());
        }
        for ext in EXT_GROUPS.iter().flat_map(|g| *g) {
            expected_files.push(format!("{dotted_prefix}.{ext}"));
        }

        let files: Vec<String> = paths
            .iter()
            .map(|p| {
                p.file_name()
                    .ok_or_else(|| anyhow!("candidate path missing file name"))
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect::<Result<_>>()?;
        ensure!(
            files == expected_files,
            "unexpected candidate ordering: {:?}",
            files
        );

        let group_len: usize = EXT_GROUPS.iter().map(|g| g.len()).sum();

        let home_parent = paths
            .first()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow!("HOME candidate must have a parent directory"))?;
        ensure!(
            paths
                .iter()
                .take(group_len)
                .all(|p| p.parent() == Some(home_parent)),
            "home candidates must share the same parent directory"
        );

        let total_len = paths.len();
        if total_len.saturating_sub(group_len) > group_len {
            let mid_slice = &paths[group_len..total_len - group_len];
            let xdg_parent = mid_slice[0]
                .parent()
                .ok_or_else(|| anyhow!("platform candidate must have a parent directory"))?;
            ensure!(
                mid_slice.iter().all(|p| p.parent() == Some(xdg_parent)),
                "platform candidates must share the same parent directory"
            );
        }

        let local_slice = paths
            .get(paths.len().saturating_sub(group_len)..)
            .unwrap_or(&[]);
        ensure!(
            local_slice.len() == group_len,
            "local candidate count must equal EXT_GROUPS size"
        );
        ensure!(
            local_slice
                .iter()
                .all(|p| p.parent() == Some(Path::new("."))),
            "local candidates must reside in the current directory"
        );

        drop(home_guard);
        Ok(())
    }
}
