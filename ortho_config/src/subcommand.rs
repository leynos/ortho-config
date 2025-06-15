#[allow(deprecated)]
use crate::merge_cli_over_defaults;
use crate::{OrthoError, load_config_file, normalize_prefix};
use clap::CommandFactory;
#[cfg(not(any(unix, target_os = "redox")))]
use directories::BaseDirs;
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use uncased::Uncased;
#[cfg(any(unix, target_os = "redox"))]
use xdg::BaseDirectories;

/// Prefix used when constructing configuration paths and environment variables.
///
/// Stores the raw prefix as provided by the user alongside a normalized
/// lowercase version used for file lookups.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Prefix {
    raw: String,
    normalized: String,
}

impl Prefix {
    /// Create a new `Prefix` from `raw`. The `raw` value is kept as-is for
    /// environment variables while a normalized version is used for file paths.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        Self {
            raw: raw.to_owned(),
            normalized: normalize_prefix(raw),
        }
    }

    #[must_use]
    fn as_str(&self) -> &str {
        &self.normalized
    }

    #[must_use]
    fn raw(&self) -> &str {
        &self.raw
    }
}

/// Name of a subcommand.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CmdName(String);

impl CmdName {
    /// Create a new command name from `raw`.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        Self(raw.to_owned())
    }

    #[must_use]
    fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    fn env_key(&self) -> String {
        self.0.replace('-', "_").to_ascii_uppercase()
    }
}

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

fn push_home_candidates(home: &Path, base: &Prefix, paths: &mut Vec<PathBuf>) {
    push_candidates(paths, &format!(".{}", base.as_str()), |f| home.join(f));
}

#[cfg(not(any(unix, target_os = "redox")))]
fn push_cfg_candidates(dir: &Path, paths: &mut Vec<PathBuf>) {
    push_candidates(paths, "config", |f| dir.join(f));
}

fn push_local_candidates(base: &Prefix, paths: &mut Vec<PathBuf>) {
    push_candidates(paths, &format!(".{}", base.as_str()), PathBuf::from);
}

/// Return possible configuration file paths for a subcommand.
fn candidate_paths(prefix: &Prefix) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(any(unix, target_os = "redox"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            push_home_candidates(Path::new(&home), prefix, &mut paths);
        }

        let xdg_dirs = if prefix.as_str().is_empty() {
            BaseDirectories::new()
        } else {
            BaseDirectories::with_prefix(prefix.as_str())
        };
        if let Some(p) = xdg_dirs.find_config_file("config.toml") {
            paths.push(p);
        }
        #[cfg(feature = "json5")]
        for ext in ["json", "json5"] {
            if let Some(p) = xdg_dirs.find_config_file(format!("config.{ext}")) {
                paths.push(p);
            }
        }
        #[cfg(feature = "yaml")]
        for ext in ["yaml", "yml"] {
            if let Some(p) = xdg_dirs.find_config_file(format!("config.{ext}")) {
                paths.push(p);
            }
        }
    }

    #[cfg(not(any(unix, target_os = "redox")))]
    {
        if let Some(dirs) = BaseDirs::new() {
            push_home_candidates(dirs.home_dir(), prefix, &mut paths);
            let cfg_dir = if prefix.as_str().is_empty() {
                dirs.config_dir().to_path_buf()
            } else {
                dirs.config_dir().join(prefix.as_str())
            };
            push_cfg_candidates(&cfg_dir, &mut paths);
        } else if let Some(home) = std::env::var_os("HOME") {
            push_home_candidates(Path::new(&home), prefix, &mut paths);
        }
    }

    push_local_candidates(prefix, &mut paths);
    paths
}

/// Load and merge `[cmds.<name>]` sections from the given paths.
#[allow(clippy::result_large_err)]
fn load_from_files(paths: &[PathBuf], name: &CmdName) -> Result<Figment, OrthoError> {
    let mut fig = Figment::new();
    for p in paths {
        if let Some(file_fig) = load_config_file(p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{}", name.as_str())));
        }
    }
    Ok(fig)
}

/// Load configuration for a specific subcommand.
///
/// The configuration is sourced from:
///   * `[cmds.<name>]` sections in configuration files
///   * environment variables following the pattern `<PREFIX>CMDS_<NAME>_`.
///
/// Values from environment variables override those from files.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
///
/// # Deprecated
///
/// Use [`load_and_merge_subcommand`] or [`load_and_merge_subcommand_for`] instead
/// to load defaults and apply CLI overrides in one step.
#[allow(clippy::result_large_err)]
#[deprecated(note = "use `load_and_merge_subcommand` or `load_and_merge_subcommand_for` instead")]
pub fn load_subcommand_config<T>(prefix: &Prefix, name: &CmdName) -> Result<T, OrthoError>
where
    T: DeserializeOwned + Default,
{
    let paths = candidate_paths(prefix);
    let mut fig = load_from_files(&paths, name)?;

    let env_name = name.env_key();
    let env_prefix = format!("{}CMDS_{env_name}_", prefix.raw());
    let env_provider = Env::prefixed(&env_prefix)
        .map(|k| Uncased::from(k))
        .split("__");
    fig = fig.merge(env_provider);

    fig.extract().map_err(OrthoError::Gathering)
}

/// Load default values for a subcommand using `T`'s configured prefix.
///
/// The prefix is provided by [`OrthoConfig::prefix`]. If the struct does not
/// specify `#[ortho_config(prefix = "...")]`, the default empty prefix is used.
/// Combine the returned defaults with CLI arguments using
/// [`merge_cli_over_defaults`](crate::merge_cli_over_defaults).
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
#[allow(clippy::result_large_err)]
pub fn load_subcommand_config_for<T>(name: &CmdName) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + Default,
{
    #[allow(deprecated)]
    {
        load_subcommand_config(&Prefix::new(T::prefix()), name)
    }
}

/// Load defaults for a subcommand and merge CLI-provided values over them.
///
/// This convenience function combines [`load_subcommand_config`] and
/// [`merge_cli_over_defaults`](crate::merge_cli_over_defaults) to reduce
/// boilerplate when working with `clap` subcommands.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
#[allow(clippy::result_large_err)]
pub fn load_and_merge_subcommand<T>(prefix: &Prefix, cli: &T) -> Result<T, OrthoError>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    let name = CmdName::new(T::command().get_name());
    #[allow(deprecated)]
    let defaults: T = load_subcommand_config(prefix, &name)?;
    #[allow(deprecated)]
    merge_cli_over_defaults(&defaults, cli).map_err(OrthoError::Gathering)
}

/// Wrapper around [`load_and_merge_subcommand`] using the struct's configured prefix.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
#[allow(clippy::result_large_err)]
pub fn load_and_merge_subcommand_for<T>(cli: &T) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    load_and_merge_subcommand(&Prefix::new(T::prefix()), cli)
}
