use crate::{OrthoError, load_config_file};
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::Path;
use uncased::Uncased;

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
    if let Some(file_fig) = load_config_file(Path::new(&dotfile))? {
        fig = fig.merge(file_fig.focus(&format!("cmds.{name}")));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = std::path::PathBuf::from(home).join(&dotfile);
        if let Some(file_fig) = load_config_file(&p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{name}")));
        }
    }

    let env_prefix = format!("{}CMDS_{}_", prefix, name.to_ascii_uppercase());
    let env_provider = Env::prefixed(&env_prefix)
        .map(|k| Uncased::new(k.as_str().to_ascii_uppercase()))
        .split("__");
    fig = fig.merge(env_provider);

    fig.extract().map_err(OrthoError::Gathering)
}
