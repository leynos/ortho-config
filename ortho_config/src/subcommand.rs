#[allow(deprecated)]
use crate::merge_cli_over_defaults;
use crate::normalize_prefix;
use crate::{OrthoError, load_config_file};
use clap::CommandFactory;
use directories::BaseDirs;
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use uncased::Uncased;
#[cfg(any(unix, target_os = "redox"))]
use xdg::BaseDirectories;

/// Return possible configuration file paths for a subcommand.
fn candidate_paths(prefix: &str) -> Vec<PathBuf> {
    let base = normalize_prefix(prefix);
    let mut paths = Vec::new();
    if let Some(dirs) = BaseDirs::new() {
        let home = dirs.home_dir();
        paths.push(home.join(format!(".{base}.toml")));
        #[cfg(feature = "json5")]
        for ext in ["json", "json5"] {
            paths.push(home.join(format!(".{base}.{ext}")));
        }
        #[cfg(feature = "yaml")]
        for ext in ["yaml", "yml"] {
            paths.push(home.join(format!(".{base}.{ext}")));
        }

        #[cfg(any(unix, target_os = "redox"))]
        {
            let xdg_dirs = if base.is_empty() {
                BaseDirectories::new()
            } else {
                BaseDirectories::with_prefix(&base)
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
            let cfg_dir = if base.is_empty() {
                dirs.config_dir().to_path_buf()
            } else {
                dirs.config_dir().join(&base)
            };
            paths.push(cfg_dir.join("config.toml"));
            #[cfg(feature = "json5")]
            for ext in ["json", "json5"] {
                paths.push(cfg_dir.join(format!("config.{ext}")));
            }
            #[cfg(feature = "yaml")]
            for ext in ["yaml", "yml"] {
                paths.push(cfg_dir.join(format!("config.{ext}")));
            }
        }
    }

    paths.push(PathBuf::from(format!(".{base}.toml")));
    #[cfg(feature = "json5")]
    for ext in ["json", "json5"] {
        paths.push(PathBuf::from(format!(".{base}.{ext}")));
    }
    #[cfg(feature = "yaml")]
    for ext in ["yaml", "yml"] {
        paths.push(PathBuf::from(format!(".{base}.{ext}")));
    }

    paths
}

/// Load and merge `[cmds.<name>]` sections from the given paths.
#[allow(clippy::result_large_err)]
fn load_from_files(paths: &[PathBuf], name: &str) -> Result<Figment, OrthoError> {
    let mut fig = Figment::new();
    for p in paths {
        if let Some(file_fig) = load_config_file(p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{name}")));
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
pub fn load_subcommand_config<T>(prefix: &str, name: &str) -> Result<T, OrthoError>
where
    T: DeserializeOwned + Default,
{
    let paths = candidate_paths(prefix);
    let mut fig = load_from_files(&paths, name)?;

    let env_name = name.replace('-', "_").to_ascii_uppercase();
    let env_prefix = format!("{prefix}CMDS_{env_name}_");
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
pub fn load_subcommand_config_for<T>(name: &str) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + Default,
{
    #[allow(deprecated)]
    {
        load_subcommand_config(T::prefix(), name)
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
pub fn load_and_merge_subcommand<T>(prefix: &str, cli: &T) -> Result<T, OrthoError>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    let name = T::command().get_name().to_owned();
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
    load_and_merge_subcommand(T::prefix(), cli)
}
