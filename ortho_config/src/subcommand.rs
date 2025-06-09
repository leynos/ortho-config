use crate::{OrthoError, load_config_file};
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::Path;
use uncased::Uncased;
use xdg::BaseDirectories;

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
    let mut fig = Figment::new();

    let dotfile = format!(
        ".{}.toml",
        prefix.trim_end_matches('_').to_ascii_lowercase()
    );
    if let Some(home) = std::env::var_os("HOME") {
        let p = std::path::PathBuf::from(home).join(&dotfile);
        if let Some(file_fig) = load_config_file(&p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{name}")));
        }
    }
    let xdg_base = prefix.trim_end_matches('_').to_ascii_lowercase();
    let xdg_dirs = if xdg_base.is_empty() {
        BaseDirectories::new()
    } else {
        BaseDirectories::with_prefix(&xdg_base)
    };
    if let Some(p) = xdg_dirs.find_config_file("config.toml") {
        if let Some(file_fig) = load_config_file(&p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{name}")));
        }
    }
    if let Some(file_fig) = load_config_file(Path::new(&dotfile))? {
        fig = fig.merge(file_fig.focus(&format!("cmds.{name}")));
    }

    let env_name = name.replace('-', "_").to_ascii_uppercase();
    let env_prefix = format!("{prefix}CMDS_{env_name}_");
    let env_provider = Env::prefixed(&env_prefix)
        .map(|k| Uncased::from(k))
        .split("__");
    fig = fig.merge(env_provider);

    fig.extract().map_err(OrthoError::Gathering)
}
