use crate::normalize_prefix;
use crate::{OrthoError, load_config_file};
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use uncased::Uncased;
use xdg::BaseDirectories;

/// Return possible configuration file paths for a subcommand.
fn candidate_paths(prefix: &str) -> Vec<PathBuf> {
    let base = normalize_prefix(prefix);
    let mut paths = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for ext in ["toml", "json5"] {
            paths.push(home.join(format!(".{base}.{ext}")));
        }
    }

    let xdg_dirs = if base.is_empty() {
        BaseDirectories::new()
    } else {
        BaseDirectories::with_prefix(&base)
    };
    for ext in ["toml", "json5"] {
        if let Some(p) = xdg_dirs.find_config_file(format!("config.{ext}")) {
            paths.push(p);
        }
    }

    for ext in ["toml", "json5"] {
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
#[allow(clippy::result_large_err)]
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
