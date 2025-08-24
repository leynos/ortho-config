//! Support for loading configuration for individual subcommands.

use crate::{OrthoError, load_config_file, sanitized_provider};
use clap::CommandFactory;
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use uncased::Uncased;

mod paths;
mod types;

use paths::candidate_paths;
pub use paths::push_stem_candidates;
pub use types::{CmdName, Prefix};

/// Load and merge `[cmds.<name>]` sections from the given paths.
///
/// For each provided path, loads the configuration file and merges the
/// `[cmds.<name>]` section into a single `Figment` instance. Only the focused
/// section for the given subcommand is merged from each file.
///
/// # Parameters
/// - `paths`: Slice of file paths to search for configuration.
/// - `name`: The subcommand name whose configuration section to load.
///
/// # Returns
/// A `Figment` instance containing the merged configuration for the subcommand,
/// or an error if loading fails.
fn load_from_files(paths: &[PathBuf], name: &CmdName) -> Result<Figment, OrthoError> {
    let mut fig = Figment::new();
    for p in paths {
        if let Some(file_fig) = load_config_file(p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{}", name.as_str())));
        }
    }
    Ok(fig)
}

/// Loads defaults for a subcommand and merges CLI-provided values over them.
///
/// This convenience function loads default configuration from files and
/// environment variables using the given prefix, then overlays values provided
/// via the CLI. CLI-provided values override file or environment defaults.
///
/// # Errors
///
/// Returns [`OrthoError::Merge`] if CLI values cannot be merged or if
/// deserialisation fails. Because CLI merging occurs, this function does not
/// return [`OrthoError::Gathering`].
///
/// # Examples
///
/// ```rust,ignore
/// use ortho_config::subcommand::load_and_merge_subcommand;
/// use ortho_config::subcommand::Prefix;
/// # use clap::Parser;
/// #[derive(clap::Parser, serde::Serialize, serde::Deserialize, Default)]
/// struct MyCmd {
///     #[clap(long)]
///     value: Option<String>,
/// }
///
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// // Assume configuration files default `value` to "file".
/// let prefix = Prefix::new("MYAPP");
/// let cli = MyCmd { value: Some("cli".to_string()) };
/// let config = load_and_merge_subcommand(&prefix, &cli)?;
/// // Assert CLI overrides defaults (assuming a file/env default of "file").
/// assert_eq!(config.value.as_deref(), Some("cli"));
/// # Ok(())
/// # }
/// ```
pub fn load_and_merge_subcommand<T>(prefix: &Prefix, cli: &T) -> Result<T, OrthoError>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    let name = CmdName::new(T::command().get_name());
    let paths = candidate_paths(prefix);
    let mut fig = load_from_files(&paths, &name)?;

    let env_name = name.env_key();
    let env_prefix = format!("{}CMDS_{env_name}_", prefix.raw());
    let env_provider = Env::prefixed(&env_prefix)
        .map(|k| Uncased::from(k))
        .split("__");
    fig = fig.merge(env_provider);

    // CLI values are merged over defaults; map failures to `Merge`.
    fig.merge(sanitized_provider(cli)?)
        .extract()
        .map_err(OrthoError::merge)
}

/// Wrapper around [`load_and_merge_subcommand`] using the struct's configured
/// prefix.
///
/// Loads default configuration values for the subcommand from files and
/// environment variables, then merges CLI-provided values over these defaults.
/// The prefix is determined by the `OrthoConfig` implementation of the type.
///
/// # Errors
///
/// Returns [`OrthoError::Merge`] if CLI values cannot be merged or if
/// deserialisation fails.
///
/// # Examples
///
/// ```rust,ignore
/// use ortho_config::subcommand::load_and_merge_subcommand_for;
/// use clap::Parser;
/// #[derive(clap::Parser, serde::Serialize, serde::Deserialize, Default)]
/// struct MyCmd { /* fields */ }
/// impl ortho_config::OrthoConfig for MyCmd {
///     fn load_and_merge(&self) -> Result<Self, ortho_config::OrthoError> where Self: serde::Serialize { todo!() }
///     fn load() -> Result<Self, ortho_config::OrthoError> { todo!() }
///     fn prefix() -> &'static str { "myapp" }
/// }
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let cli = MyCmd::parse_from(&["mycmd"]);
/// let config = load_and_merge_subcommand_for(&cli)?;
/// # let _ = config;
/// # Ok(())
/// # }
/// ```
pub fn load_and_merge_subcommand_for<T>(cli: &T) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    load_and_merge_subcommand(&Prefix::new(T::prefix()), cli)
}

/// Trait adding a convenience [`load_and_merge`] method to subcommand structs.
///
/// Implemented for any type that satisfies the bounds required by
/// [`load_and_merge_subcommand_for`]. This avoids writing identical
/// `load_and_merge` methods for each subcommand struct in an application.
///
/// # Examples
///
/// ```rust,no_run
/// use clap::Parser;
/// use ortho_config::{OrthoConfig, subcommand::SubcmdConfigMerge};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Parser, Deserialize, Serialize, OrthoConfig, Default)]
/// #[ortho_config(prefix = "APP_")]
/// struct RunArgs {
///     #[arg(long)]
///     level: Option<u32>,
/// }
///
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let cli = RunArgs::parse_from(["tool", "--level", "3"]);
/// let cfg = cli.load_and_merge()?;
/// # let _ = cfg;
/// # Ok(())
/// # }
/// ```
pub trait SubcmdConfigMerge:
    crate::OrthoConfig + serde::Serialize + Default + CommandFactory + Sized
{
    /// Merge configuration defaults for this subcommand over CLI arguments.
    ///
    /// Loads defaults from configuration files and the environment, then
    /// overlays the already parsed CLI values.
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError::Merge`] if CLI values cannot be merged or if
    /// deserialisation fails.
    fn load_and_merge(&self) -> Result<Self, OrthoError> {
        load_and_merge_subcommand_for(self)
    }
}

impl<T> SubcmdConfigMerge for T where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory
{
}
