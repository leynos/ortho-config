//! Utilities for discovering configuration file paths for subcommands.
//!
//! Enumerates candidate configuration files under the user's home directory,
//! platform-specific configuration directories, and an explicit local base.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::types::Prefix;
#[cfg(any(unix, target_os = "redox"))]
use crate::OrthoError;
use crate::{EnvSource, OrthoResult};

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
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// use ortho_config::subcommand::push_stem_candidates;
///
/// let directory = Path::new("config");
/// let mut candidates = Vec::new();
/// push_stem_candidates(directory, ".myapp", &mut candidates);
///
/// assert!(candidates.contains(&directory.join(".myapp.toml")));
/// ```
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

/// Adds the first existing XDG configuration candidate for each file type.
///
/// Missing candidates are skipped, while other metadata lookup failures are
/// returned so an inaccessible XDG location cannot silently change discovery.
///
/// # Errors
///
/// Returns an I/O error for an XDG candidate when its existence cannot be
/// determined for a reason other than the candidate being absent.
#[cfg(any(unix, target_os = "redox"))]
fn push_xdg_candidates(
    prefix: &Prefix,
    source: &dyn EnvSource,
    paths: &mut Vec<PathBuf>,
) -> OrthoResult<()> {
    let bases = source_xdg_bases(prefix, source);
    for ext in EXT_GROUPS.iter().flat_map(|group| *group) {
        let file = format!("config.{ext}");
        let mut existing_path = None;
        for base in &bases {
            let path = base.join(&file);
            let exists = match path.try_exists() {
                Ok(exists) => exists,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(io_error) => {
                    return Err(Arc::new(OrthoError::File {
                        path,
                        source: Box::new(io_error),
                    }));
                }
            };
            if exists {
                existing_path = Some(path);
                break;
            }
        }
        if let Some(path) = existing_path {
            paths.push(path);
        }
    }
    Ok(())
}

/// Adds home and XDG candidates to the subcommand search path.
///
/// # Errors
///
/// Returns metadata lookup failures from checking XDG candidates, preserving
/// the path that could not be inspected.
#[cfg(any(unix, target_os = "redox"))]
fn collect_unix_paths(
    prefix: &Prefix,
    source: &dyn EnvSource,
    paths: &mut Vec<PathBuf>,
) -> OrthoResult<()> {
    if let Some(home) = source.get("HOME") {
        push_stem_candidates(Path::new(&home), &dotted(prefix), paths);
    }
    push_xdg_candidates(prefix, source, paths)
}

#[cfg(not(any(unix, target_os = "redox")))]
fn collect_non_unix_paths(prefix: &Prefix, source: &dyn EnvSource, paths: &mut Vec<PathBuf>) {
    let configured_home = source.get("HOME").or_else(|| source.get("USERPROFILE"));
    if let Some(home) = configured_home {
        push_stem_candidates(Path::new(&home), &dotted(prefix), paths);
    } else if let Some(fallback_home) = source.home_fallback() {
        push_stem_candidates(&fallback_home, &dotted(prefix), paths);
    }

    if let Some(config_dir) = source.config_dir_fallback() {
        let scoped_config_dir = if prefix.as_str().is_empty() {
            config_dir
        } else {
            config_dir.join(prefix.as_str())
        };
        push_stem_candidates(&scoped_config_dir, "config", paths);
    }
}

/// Returns candidate paths for an explicit base and named environment source.
///
/// On Unix and Redox, this may inspect XDG candidates while constructing the
/// path list. An explicitly supplied `base` is always added as the local
/// candidate root.
///
/// # Errors
///
/// On Unix and Redox, returns an I/O error if an XDG candidate cannot be
/// inspected. A missing candidate is not an error.
pub(super) fn candidate_paths_at(
    prefix: &Prefix,
    base: &Path,
    source: &dyn EnvSource,
) -> OrthoResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    #[cfg(any(unix, target_os = "redox"))]
    collect_unix_paths(prefix, source, &mut paths)?;
    #[cfg(not(any(unix, target_os = "redox")))]
    collect_non_unix_paths(prefix, source, &mut paths);
    push_local_candidates(prefix, base, &mut paths);
    Ok(paths)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
