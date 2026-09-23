//! Utilities for discovering configuration file paths for subcommands.
//!
//! Enumerates candidate configuration files under the user's home directory,
//! platform-specific configuration directories, and an explicit local base.

use std::path::{Path, PathBuf};

use super::types::Prefix;
use crate::EnvSource;

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
    let prefix_name = prefix.as_str();
    if prefix_name.is_empty() {
        String::new()
    } else {
        format!(".{prefix_name}")
    }
}

/// Adds candidate configuration file paths under `dir` using `base` as the file stem.
///
/// The `base` string should include any desired prefix such as a leading dot.
pub fn push_stem_candidates(dir: &Path, base: &str, paths: &mut Vec<PathBuf>) {
    push_candidates(paths, base, |file| dir.join(file));
}

fn push_local_candidates(prefix: &Prefix, base: &Path, paths: &mut Vec<PathBuf>) {
    push_stem_candidates(base, &dotted(prefix), paths);
}

#[cfg(any(unix, target_os = "redox"))]
fn source_xdg_bases(prefix: &Prefix, source: &dyn EnvSource) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    let prefix_path = Path::new(prefix.as_str());
    let config_home_result = source
        .get("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            source
                .get("HOME")
                .map(PathBuf::from)
                .or_else(|| source.home_fallback())
                .map(|home| home.join(".config"))
        });
    if let Some(config_home) = config_home_result {
        bases.push(config_home.join(prefix_path));
    }

    let config_dirs = source
        .get("XDG_CONFIG_DIRS")
        .map(|dirs| {
            std::env::split_paths(&dirs)
                .filter(|path| path.is_absolute())
                .collect::<Vec<_>>()
        })
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| vec![PathBuf::from("/etc/xdg")]);
    bases.extend(config_dirs.into_iter().map(|dir| dir.join(prefix_path)));
    bases
}

#[cfg(any(unix, target_os = "redox"))]
fn push_xdg_candidates(prefix: &Prefix, source: &dyn EnvSource, paths: &mut Vec<PathBuf>) {
    let bases = source_xdg_bases(prefix, source);
    for group in EXT_GROUPS {
        for ext in *group {
            let file = format!("config.{ext}");
            if let Some(path) = bases
                .iter()
                .map(|base| base.join(&file))
                .find(|path| path.try_exists().unwrap_or(false))
            {
                paths.push(path);
            }
        }
    }
}

#[cfg(any(unix, target_os = "redox"))]
fn collect_unix_paths(prefix: &Prefix, source: &dyn EnvSource, paths: &mut Vec<PathBuf>) {
    if let Some(home) = source.get("HOME") {
        push_stem_candidates(Path::new(&home), &dotted(prefix), paths);
    }
    push_xdg_candidates(prefix, source, paths);
}

#[cfg(not(any(unix, target_os = "redox")))]
fn collect_non_unix_paths(prefix: &Prefix, source: &dyn EnvSource, paths: &mut Vec<PathBuf>) {
    let home = source.get("HOME").or_else(|| source.get("USERPROFILE"));
    if let Some(home) = home {
        push_stem_candidates(Path::new(&home), &dotted(prefix), paths);
    } else if let Some(home) = source.home_fallback() {
        push_stem_candidates(&home, &dotted(prefix), paths);
    }

    if let Some(config_dir) = source.config_dir_fallback() {
        let config_dir = if prefix.as_str().is_empty() {
            config_dir
        } else {
            config_dir.join(prefix.as_str())
        };
        push_stem_candidates(&config_dir, "config", paths);
    }
}

/// Returns candidate paths for an explicit base and named environment source.
pub(super) fn candidate_paths_at(
    prefix: &Prefix,
    base: &Path,
    source: &dyn EnvSource,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(any(unix, target_os = "redox"))]
    collect_unix_paths(prefix, source, &mut paths);
    #[cfg(not(any(unix, target_os = "redox")))]
    collect_non_unix_paths(prefix, source, &mut paths);
    push_local_candidates(prefix, base, &mut paths);
    paths
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
