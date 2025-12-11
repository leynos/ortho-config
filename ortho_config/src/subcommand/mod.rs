//! Utilities for loading configuration for individual subcommands.
//!
//! Resolves defaults from files and the environment and exposes the
//! [`SubcmdConfigMerge`] trait for merging them with CLI arguments.

#[cfg(feature = "serde_json")]
use crate::{CliValueExtractor, OrthoMergeExt, OrthoResult, load_config_file, sanitized_provider};
#[cfg(feature = "serde_json")]
use clap::{ArgMatches, CommandFactory};
#[cfg(feature = "serde_json")]
use figment::providers::Serialized;
#[cfg(feature = "serde_json")]
use figment::{Figment, providers::Env};
#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;
#[cfg(feature = "serde_json")]
use std::path::PathBuf;
#[cfg(feature = "serde_json")]
use uncased::Uncased;

mod paths;
mod types;

#[cfg(feature = "serde_json")]
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
#[cfg(feature = "serde_json")]
fn load_from_files(paths: &[PathBuf], name: &CmdName) -> OrthoResult<Figment> {
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
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or if
/// deserialization fails. Because CLI merging occurs, this function does not
/// return [`crate::OrthoError::Gathering`].
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::subcommand::load_and_merge_subcommand;
/// use ortho_config::subcommand::Prefix;
/// # use clap::Parser;
/// #[derive(clap::Parser, serde::Serialize, serde::Deserialize, Default)]
/// struct MyCmd {
///     #[clap(long)]
///     value: Option<String>,
/// }
///
/// # fn main() -> ortho_config::OrthoResult<()> {
/// // Assume configuration files default `value` to "file".
/// let prefix = Prefix::new("MYAPP");
/// let cli = MyCmd { value: Some("cli".to_string()) };
/// let config = load_and_merge_subcommand(&prefix, &cli)?;
/// // Assert CLI overrides defaults (assuming a file/env default of "file").
/// assert_eq!(config.value.as_deref(), Some("cli"));
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand<T>(prefix: &Prefix, cli: &T) -> OrthoResult<T>
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
        .into_ortho_merge()
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
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or if
/// deserialization fails.
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
/// # fn main() -> ortho_config::OrthoResult<()> {
/// let cli = MyCmd::parse_from(&["mycmd"]);
/// let config = load_and_merge_subcommand_for(&cli)?;
/// # let _ = config;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for<T>(cli: &T) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    load_and_merge_subcommand(&Prefix::new(T::prefix()), cli)
}

/// Loads defaults for a subcommand, respecting `cli_default_as_absent` fields.
///
/// This variant uses [`CliValueExtractor`] to distinguish between values
/// explicitly provided on the CLI and clap's default values. Fields marked with
/// `#[ortho_config(cli_default_as_absent)]` are excluded from the CLI merge
/// layer unless the user explicitly provided them, allowing file and environment
/// configuration to take precedence.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or if
/// deserialisation fails.
///
/// # Examples
///
/// ```rust,no_run
/// use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser};
/// use ortho_config::{OrthoConfig, OrthoResult};
/// use ortho_config::subcommand::{Prefix, load_and_merge_subcommand_with_matches};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Parser, Deserialize, Serialize, OrthoConfig, Default)]
/// #[ortho_config(prefix = "APP_")]
/// struct MyCmd {
///     #[arg(long, default_value_t = String::from("default"))]
///     #[ortho_config(default = String::from("default"), cli_default_as_absent)]
///     value: String,
/// }
///
/// # fn main() -> OrthoResult<()> {
/// let matches = MyCmd::command().get_matches();
/// let cli = MyCmd::from_arg_matches(&matches).expect("parse failed");
/// let prefix = Prefix::new("APP_");
/// let config = load_and_merge_subcommand_with_matches(&prefix, &cli, &matches)?;
/// // config.value comes from file/env if user didn't pass --value
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_with_matches<T>(
    prefix: &Prefix,
    cli: &T,
    matches: &ArgMatches,
) -> OrthoResult<T>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory + CliValueExtractor,
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

    // Extract only user-provided CLI values, respecting cli_default_as_absent
    let cli_value = cli.extract_user_provided(matches)?;
    fig.merge(Serialized::defaults(cli_value))
        .extract()
        .into_ortho_merge()
}

/// Wrapper around [`load_and_merge_subcommand_with_matches`] using the struct's
/// configured prefix.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or if
/// deserialisation fails.
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_matches<T>(
    cli: &T,
    matches: &ArgMatches,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory + CliValueExtractor,
{
    load_and_merge_subcommand_with_matches(&Prefix::new(T::prefix()), cli, matches)
}

/// Trait adding a convenience [`SubcmdConfigMerge::load_and_merge`] method to subcommand structs.
///
/// Implemented for any type that satisfies the bounds required by
/// [`load_and_merge_subcommand_for`]. This avoids writing identical
/// `load_and_merge` methods for each subcommand struct in an application.
///
/// # Examples
///
/// ```rust,no_run
/// use clap::Parser;
/// use ortho_config::OrthoConfig;
/// use ortho_config::SubcmdConfigMerge;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Parser, Deserialize, Serialize, OrthoConfig, Default)]
/// #[ortho_config(prefix = "APP_")]
/// struct RunArgs {
///     #[arg(long)]
///     level: Option<u32>,
/// }
///
/// # fn main() -> ortho_config::OrthoResult<()> {
/// let cli = RunArgs::parse_from(["tool", "--level", "3"]);
/// let cfg = cli.load_and_merge()?;
/// # let _ = cfg;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub trait SubcmdConfigMerge: crate::OrthoConfig + CommandFactory + Sized {
    /// Merge configuration defaults for this subcommand over CLI arguments.
    ///
    /// Loads defaults from configuration files and the environment, then
    /// overlays the already parsed CLI values.
    ///
    /// # Errors
    ///
    /// Returns an [`crate::OrthoError::Merge`] if CLI values cannot be merged or if
    /// deserialisation fails.
    fn load_and_merge(&self) -> OrthoResult<Self>
    where
        Self: serde::Serialize + Default,
    {
        load_and_merge_subcommand_for(self)
    }

    /// Merge configuration defaults, respecting `cli_default_as_absent` fields.
    ///
    /// This variant uses the provided `ArgMatches` to distinguish between
    /// values explicitly provided on the CLI and clap's default values.
    /// Fields marked with `#[ortho_config(cli_default_as_absent)]` are excluded
    /// from the CLI merge layer unless the user explicitly provided them.
    ///
    /// # Errors
    ///
    /// Returns an [`crate::OrthoError::Merge`] if CLI values cannot be merged or if
    /// deserialisation fails.
    fn load_and_merge_with_matches(&self, matches: &ArgMatches) -> OrthoResult<Self>
    where
        Self: serde::Serialize + Default + CliValueExtractor,
    {
        load_and_merge_subcommand_for_with_matches(self, matches)
    }
}

#[cfg(feature = "serde_json")]
impl<T> SubcmdConfigMerge for T where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory
{
}
