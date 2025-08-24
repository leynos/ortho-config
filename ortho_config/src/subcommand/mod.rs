//! Support for loading configuration for individual subcommands.

use crate::{OrthoError, load_config_file, sanitized_provider};
use clap::CommandFactory;
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use uncased::{Uncased, UncasedStr};

mod paths;
mod types;

use paths::candidate_paths;
pub use paths::push_stem_candidates;
pub use types::{CmdName, Prefix};

/// Maps an `UncasedStr` to an owned `Uncased` without using an inline closure.
fn to_uncased(key: &UncasedStr) -> Uncased<'_> {
    Uncased::from(key)
}

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

/// Loads configuration for a specific subcommand from files and environment
/// variables.
///
/// Searches for configuration files using the provided prefix, loads the
/// `[cmds.<name>]` section from each file, and merges them. Then overlays
/// environment variables prefixed with `<PREFIX>CMDS_<NAME>_` (case-insensitive,
/// double underscore for nesting). Values from environment variables override
/// those from files.
///
/// # Errors
///
/// Returns [`OrthoError::Gathering`] if configuration files cannot be loaded or
/// if deserialisation fails.
///
/// # Deprecated
///
/// Use [`load_and_merge_subcommand`] or
/// [`load_and_merge_subcommand_for`] instead to load defaults and apply CLI
/// overrides in one step. Planned removal: v0.4.0 (see the project roadmap).
#[deprecated(
    since = "0.3.0",
    note = "use `load_and_merge_subcommand` or `load_and_merge_subcommand_for` instead; removed in v0.4.0"
)]
pub fn load_subcommand_config<T>(prefix: &Prefix, name: &CmdName) -> Result<T, OrthoError>
where
    T: DeserializeOwned + Default,
{
    let paths = candidate_paths(prefix);
    let mut fig = load_from_files(&paths, name)?;

    let env_name = name.env_key();
    let env_prefix = format!("{}CMDS_{env_name}_", prefix.raw());
    let env_provider = Env::prefixed(&env_prefix).map(to_uncased).split("__");
    fig = fig.merge(env_provider);

    // Extraction only gathers defaults, so map failures accordingly.
    fig.extract().map_err(|e| OrthoError::Gathering(e.into()))
}

/// Loads configuration defaults for a subcommand using the prefix defined by the
/// type.
///
/// The prefix is provided by [`OrthoConfig::prefix`]. If the struct does not
/// specify `#[ortho_config(prefix = "...")]`, the default empty prefix is used.
/// This function loads `[cmds.<name>]` sections from configuration files and
/// overlays environment variables prefixed with `<PREFIX>CMDS_<NAME>_`,
/// returning the merged configuration as type `T`.
///
/// # Errors
///
/// Returns [`OrthoError::Gathering`] if configuration files cannot be loaded or
/// if deserialisation fails.
///
/// # Examples
///
/// ```rust,ignore
/// use ortho_config::subcommand::{CmdName, load_subcommand_config_for};
/// #[derive(clap::Parser, serde::Serialize, serde::Deserialize, Default)]
/// struct MySubcommandConfig;
/// impl ortho_config::OrthoConfig for MySubcommandConfig {
///     fn load_and_merge(&self) -> Result<Self, ortho_config::OrthoError> where Self: serde::Serialize { todo!() }
///     fn load() -> Result<Self, ortho_config::OrthoError> { todo!() }
///     fn prefix() -> &'static str { "" }
/// }
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let config = load_subcommand_config_for::<MySubcommandConfig>(&CmdName::new("serve"))?;
/// # let _ = config;
/// # Ok(())
/// # }
/// ```
///
/// # Deprecated
///
/// This function is deprecated. Use
/// [`load_and_merge_subcommand_for`](crate::load_and_merge_subcommand_for)
/// instead. Planned removal: v0.4.0 (see the project roadmap).
#[deprecated(
    since = "0.3.0",
    note = "use `load_and_merge_subcommand_for` instead; removed in v0.4.0"
)]
pub fn load_subcommand_config_for<T>(name: &CmdName) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + Default,
{
    #[expect(
        deprecated,
        reason = "delegates to deprecated helper during transition"
    )]
    load_subcommand_config(&Prefix::new(T::prefix()), name)
}

/// Loads defaults for a subcommand and merges CLI-provided values over them.
///
/// This convenience function combines [`load_subcommand_config`] with CLI
/// overrides to reduce boilerplate when working with `clap` subcommands. It determines the
/// subcommand name from `T`, loads default configuration from files and
/// environment variables using the given prefix, and overlays values provided
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
