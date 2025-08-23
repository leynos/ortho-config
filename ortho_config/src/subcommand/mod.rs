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
#[expect(
    clippy::result_large_err,
    reason = "Figment merge errors inflate Result size; wrapping in `Arc` is tracked on the roadmap for v0.4.0"
)]
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
/// Returns an [`OrthoError`] if file loading or deserialisation fails.
///
/// # Deprecated
///
/// Use [`load_and_merge_subcommand`] or [`load_and_merge_subcommand_for`] instead
/// to load defaults and apply CLI overrides in one step.
#[expect(
    clippy::result_large_err,
    reason = "Figment merge errors inflate Result size; wrapping in `Arc` is tracked on the roadmap for v0.4.0"
)]
#[deprecated(note = "use `load_and_merge_subcommand` or `load_and_merge_subcommand_for` instead")]
/// Loads configuration for a specific subcommand from files and environment variables.
///
/// Searches for configuration files using the provided prefix, loads the `[cmds.<name>]`
/// section from each file, and merges them. Then overlays environment variables
/// prefixed with `<PREFIX>CMDS_<NAME>_` (case-insensitive, double underscore for
/// nesting). Returns the merged configuration as type `T`.
///
/// # Deprecated
///
/// This function is deprecated. Use the newer combined loading and merging functions instead. Removal is tracked on the project roadmap.
///
/// # Errors
///
/// Returns an error if configuration files cannot be loaded or if deserialisation fails.
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
/// Returns an [`OrthoError`] if file loading or deserialisation fails.
#[expect(
    clippy::result_large_err,
    reason = "Figment merge errors inflate Result size; wrapping in `Arc` is tracked on the roadmap for v0.4.0"
)]
#[deprecated(note = "use `load_and_merge_subcommand_for` instead")]
/// Loads configuration defaults for a subcommand using the prefix defined by the type.
///
/// This function retrieves the configuration for the specified subcommand, using the prefix
/// provided by the `OrthoConfig` implementation of `T`. It loads and merges configuration
/// from files and environment variables, returning the resulting configuration as type `T`.
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
/// instead. Removal is tracked on the project roadmap.
pub fn load_subcommand_config_for<T>(name: &CmdName) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + Default,
{
    #[expect(
        deprecated,
        reason = "Call deprecated helper for backwards compatibility; removal tracked on the roadmap"
    )]
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
/// Returns an [`OrthoError`] if file loading or deserialisation fails.
#[expect(
    clippy::result_large_err,
    reason = "Figment merge errors inflate Result size; wrapping in `Arc` is tracked on the roadmap for v0.4.0"
)]
/// Loads configuration defaults for a subcommand and merges CLI-provided values over them.
///
/// This function determines the subcommand name from the type `T`, loads its default configuration from files and environment
/// variables using the given prefix, and overlays values provided via the CLI. The resulting configuration is returned as type
/// `T`.
///
/// # Returns
///
/// The merged configuration for the subcommand, or an error if loading or merging fails.
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
/// let prefix = Prefix::new("MYAPP");
/// let cli = MyCmd { value: Some("cli".to_string()) };
/// let config = load_and_merge_subcommand(&prefix, &cli)?;
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

    fig.merge(sanitized_provider(cli)?)
        .extract()
        .map_err(OrthoError::merge)
}

/// Wrapper around [`load_and_merge_subcommand`] using the struct's configured prefix.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialisation fails.
#[expect(
    clippy::result_large_err,
    reason = "Figment merge errors inflate Result size; wrapping in `Arc` is tracked on the roadmap for v0.4.0"
)]
/// Loads and merges configuration for a subcommand using the prefix defined by its type.
///
/// Loads default configuration values for the subcommand from files and environment variables,
/// then merges CLI-provided values over these defaults. The prefix is determined by the
/// `OrthoConfig` implementation of the type.
///
/// # Returns
///
/// The merged configuration for the subcommand, or an error if loading or merging fails.
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
