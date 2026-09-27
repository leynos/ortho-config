//! Utilities for discovering configuration file paths for subcommands.
//!
//! Enumerates candidate configuration files under the user's home directory,
//! platform-specific configuration directories, and an explicit local base.

use std::path::{Path, PathBuf};
#[cfg(any(unix, target_os = "redox"))]
use std::sync::Arc;

use super::types::Prefix;
#[cfg(any(unix, target_os = "redox"))]
use crate::OrthoError;
use crate::{EnvSource, OrthoResult};

#[path = "paths_telemetry.rs"]
mod paths_telemetry;

const EXT_GROUPS: &[&[&str]] = &[
    &["toml"],
    #[cfg(feature = "json5")]
    &["json", "json5"],
    #[cfg(feature = "yaml")]
    &["yaml", "yml"],
];

/// Append one candidate per supported extension, grouped so aliases stay adjacent.
///
/// `EXT_GROUPS` orders the groups, and each group's extensions keep their
/// listed order, so `toml` always precedes the optional alternatives.
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

/// Render the prefix as a leading-dot file stem, or nothing when it is empty.
///
/// An empty prefix yields an empty string rather than a bare `"."`, so the
/// caller's `format!` produces `config.toml` instead of `.config.toml`.
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

/// Append the explicit-base candidates, which always follow the home and XDG ones.
fn push_local_candidates(prefix: &Prefix, base: &Path, paths: &mut Vec<PathBuf>) {
    push_stem_candidates(base, &dotted(prefix), paths);
}

/// Resolve the ordered XDG base directories that the prefix is scoped under.
///
/// The configuration home leads, followed by every absolute
/// `XDG_CONFIG_DIRS` entry in its listed order. An unusable
/// `XDG_CONFIG_HOME` falls back to `$HOME/.config` and then to
/// [`EnvSource::home_fallback`]; an unusable or absent `XDG_CONFIG_DIRS`
/// falls back to `/etc/xdg`. The prefix is appended to every base, so the
/// caller joins only the file name.
#[cfg(any(unix, target_os = "redox"))]
fn source_xdg_bases(prefix: &Prefix, source: &dyn EnvSource) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    let prefix_path = Path::new(prefix.as_str());
    let absolute_config_home = source
        .get("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let config_home_result = absolute_config_home
        .inspect(|_| paths_telemetry::unix_config_home_from_named())
        .or_else(|| {
            source
                .get("HOME")
                .map(PathBuf::from)
                .or_else(|| source.home_fallback())
                .map(|home| home.join(".config"))
                .inspect(|_| paths_telemetry::unix_config_home_from_fallback())
        });
    match config_home_result {
        Some(config_home) => bases.push(config_home.join(prefix_path)),
        None => paths_telemetry::unix_config_home_absent(),
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

/// Report whether an XDG candidate exists, keeping only `NotFound` as absence.
///
/// An unreadable parent directory or a permission failure says nothing about
/// whether the candidate is present. Collapsing such an error into "absent"
/// silently changes which configuration file loads, so it is reported against
/// the affected path instead.
#[cfg(any(unix, target_os = "redox"))]
fn xdg_candidate_exists(path: &Path) -> OrthoResult<bool> {
    match path.try_exists() {
        Ok(exists) => Ok(exists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Arc::new(OrthoError::File {
            path: path.to_path_buf(),
            source: Box::new(error),
        })),
    }
}

/// Select the first existing `name` across the ordered XDG base list.
#[cfg(any(unix, target_os = "redox"))]
fn first_existing_xdg_candidate(bases: &[PathBuf], name: &str) -> OrthoResult<Option<PathBuf>> {
    for (index, base) in bases.iter().enumerate() {
        let candidate = base.join(name);
        match xdg_candidate_exists(&candidate) {
            Ok(true) => {
                paths_telemetry::candidate_exists(index);
                return Ok(Some(candidate));
            }
            Ok(false) => {}
            Err(error) => {
                paths_telemetry::candidate_probe_failed();
                return Err(error);
            }
        }
    }
    paths_telemetry::candidate_absent();
    Ok(None)
}

/// Append the first existing XDG configuration file for each supported extension.
///
/// # Errors
///
/// Returns [`crate::OrthoError::File`] when probing a candidate path fails for
/// any reason other than the path being absent.
#[cfg(any(unix, target_os = "redox"))]
fn push_xdg_candidates(
    prefix: &Prefix,
    source: &dyn EnvSource,
    paths: &mut Vec<PathBuf>,
) -> OrthoResult<()> {
    let bases = source_xdg_bases(prefix, source);
    for group in EXT_GROUPS {
        for ext in *group {
            let file = format!("config.{ext}");
            if let Some(path) = first_existing_xdg_candidate(&bases, &file)? {
                paths.push(path);
            }
        }
    }
    Ok(())
}

/// Append the Unix home candidates followed by the XDG candidates.
///
/// # Errors
///
/// Returns [`crate::OrthoError::File`] when an XDG candidate cannot be probed.
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

/// Append the platform candidates: the home directory, then the configuration
/// directory.
///
/// The configuration directory falls back to
/// [`EnvSource::config_dir_fallback`] when the native one is unavailable, and
/// is scoped by the prefix only when the prefix is non-empty.
#[cfg(not(any(unix, target_os = "redox")))]
fn collect_non_unix_paths(prefix: &Prefix, source: &dyn EnvSource, paths: &mut Vec<PathBuf>) {
    let configured_home = source.get("HOME").or_else(|| {
        let from_userprofile = source.get("USERPROFILE");
        if from_userprofile.is_some() {
            paths_telemetry::windows_home_from_userprofile();
        }
        from_userprofile
    });
    if let Some(home) = configured_home {
        push_stem_candidates(Path::new(&home), &dotted(prefix), paths);
    } else if let Some(fallback_home) = source.home_fallback() {
        paths_telemetry::windows_home_from_named();
        push_stem_candidates(&fallback_home, &dotted(prefix), paths);
    } else {
        paths_telemetry::windows_home_absent();
    }

    if let Some(config_dir) = source.config_dir_fallback() {
        paths_telemetry::windows_config_dir_from_platform();
        let scoped_config_dir = if prefix.as_str().is_empty() {
            config_dir
        } else {
            config_dir.join(prefix.as_str())
        };
        push_stem_candidates(&scoped_config_dir, "config", paths);
    } else {
        paths_telemetry::windows_config_dir_absent();
    }
}

/// Returns candidate paths for an explicit base and named environment source.
///
/// # Errors
///
/// Returns [`crate::OrthoError::File`] when an XDG candidate cannot be probed
/// for existence. A missing candidate is not an error: only a probe that fails
/// for some other reason is reported, against the path that failed.
pub(super) fn candidate_paths_at(
    prefix: &Prefix,
    base: &Path,
    source: &dyn EnvSource,
) -> OrthoResult<Vec<PathBuf>> {
    paths_telemetry::candidates_started();
    let mut paths = Vec::new();
    let result = (|| {
        #[cfg(any(unix, target_os = "redox"))]
        collect_unix_paths(prefix, source, &mut paths)?;
        #[cfg(not(any(unix, target_os = "redox")))]
        collect_non_unix_paths(prefix, source, &mut paths);
        push_local_candidates(prefix, base, &mut paths);
        Ok(paths)
    })();
    paths_telemetry::candidates_finished(&result);
    result
}

#[cfg(test)]
#[path = "paths_probe_tests.rs"]
mod probe_tests;

#[cfg(test)]
#[path = "paths_proptests.rs"]
mod proptests;

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;

#[cfg(all(test, any(unix, target_os = "redox")))]
#[path = "paths_xdg_tests.rs"]
mod xdg_tests;
