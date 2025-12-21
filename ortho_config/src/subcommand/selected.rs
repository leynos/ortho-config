//! Helpers for merging the selected clap subcommand configuration.
//!
//! Applications often parse a root CLI struct with `clap` and then need to load
//! configuration defaults for the selected subcommand from the `[cmds.<name>]`
//! namespace (plus `PREFIX_CMDS_<NAME>_*` environment variables). Doing this at
//! the call-site tends to produce repetitive `match` scaffolding that mirrors
//! the subcommand enum variants.
//!
//! This module provides:
//! - [`SelectedSubcommandMerge`], implemented by derive on a `clap::Subcommand`
//!   enum, to merge the selected subcommand in one call; and
//! - [`load_globals_and_merge_selected_subcommand`], a small convenience helper
//!   that couples global configuration loading with subcommand merging so
//!   callers can resolve both in a single expression.

#[cfg(feature = "serde_json")]
use clap::ArgMatches;

#[cfg(feature = "serde_json")]
use std::sync::Arc;

#[cfg(feature = "serde_json")]
use thiserror::Error;

#[cfg(feature = "serde_json")]
use crate::OrthoError;

/// Errors raised while merging configuration for a selected subcommand enum.
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SelectedSubcommandMergeError {
    /// The selected enum variant requires access to the subcommand `ArgMatches`,
    /// but the provided `ArgMatches` tree contained no subcommand.
    #[error("missing clap ArgMatches for the selected subcommand ({selected})")]
    MissingSubcommandMatches {
        /// Name of the selected enum variant.
        selected: &'static str,
    },

    /// Merging defaults beneath the CLI values failed.
    #[error(transparent)]
    Merge(#[from] Arc<OrthoError>),
}

/// Trait for merging configuration defaults for the selected subcommand enum.
///
/// Prefer deriving this trait on your `clap::Subcommand` enum. The derive
/// generates the internal match that maps each variant to its corresponding
/// `load_and_merge()` (or `load_and_merge_with_matches()`) call.
///
/// # Examples
///
/// ```rust,no_run
/// use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
/// use ortho_config::{OrthoConfig, SelectedSubcommandMerge};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Parser)]
/// struct Cli {
///     #[command(subcommand)]
///     command: Commands,
/// }
///
/// #[derive(Debug, Subcommand, ortho_config_macros::SelectedSubcommandMerge)]
/// enum Commands {
///     Run(RunArgs),
/// }
///
/// #[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default)]
/// #[command(name = "run")]
/// #[ortho_config(prefix = "APP_")]
/// struct RunArgs {
///     #[arg(long)]
///     level: Option<u8>,
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cmd = Cli::command();
/// let matches = cmd.get_matches();
/// let cli = Cli::from_arg_matches(&matches)?;
/// let _merged = cli.command.load_and_merge_selected(&matches)?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub trait SelectedSubcommandMerge: Sized {
    /// Merges defaults for the selected subcommand beneath its CLI values.
    ///
    /// # Errors
    ///
    /// Returns [`SelectedSubcommandMergeError::Merge`] when file/environment
    /// defaults cannot be merged. Variants that opt into `ArgMatches`-aware
    /// merging may also return
    /// [`SelectedSubcommandMergeError::MissingSubcommandMatches`] if the caller
    /// provided a matches tree without a selected subcommand.
    fn load_and_merge_selected(
        self,
        matches: &ArgMatches,
    ) -> Result<Self, SelectedSubcommandMergeError>;
}

/// Errors raised by [`load_globals_and_merge_selected_subcommand`].
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LoadGlobalsAndSelectedSubcommandError<GlobalError>
where
    GlobalError: std::error::Error + Send + Sync + 'static,
{
    /// Global configuration loading failed.
    #[error("failed to load global configuration: {0}")]
    Globals(#[source] GlobalError),

    /// Selected subcommand merging failed.
    #[error(transparent)]
    Subcommand(#[from] SelectedSubcommandMergeError),
}

/// Loads global configuration and merges configuration defaults for the selected subcommand.
///
/// This helper exists to reduce boilerplate in entry points:
/// callers often need to resolve global configuration and the selected
/// subcommand configuration together, but the subcommand merge depends on the
/// already-parsed `ArgMatches`.
///
/// # Examples
///
/// ```rust,no_run
/// use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
/// use ortho_config::{OrthoConfig, SelectedSubcommandMerge, load_globals_and_merge_selected_subcommand};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Parser)]
/// struct Cli {
///     #[command(subcommand)]
///     command: Commands,
/// }
///
/// #[derive(Debug, Subcommand, ortho_config_macros::SelectedSubcommandMerge)]
/// enum Commands {
///     Run(RunArgs),
/// }
///
/// #[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default)]
/// #[command(name = "run")]
/// #[ortho_config(prefix = "APP_")]
/// struct RunArgs {
///     #[arg(long)]
///     level: Option<u8>,
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cmd = Cli::command();
/// let matches = cmd.get_matches();
/// let cli = Cli::from_arg_matches(&matches)?;
/// let (globals, merged) = load_globals_and_merge_selected_subcommand(
///     &matches,
///     cli.command,
///     || Ok::<_, std::io::Error>(()),
/// )?;
/// let _ = globals;
/// let _ = merged;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns [`LoadGlobalsAndSelectedSubcommandError::Globals`] when `load_globals`
/// fails, or [`LoadGlobalsAndSelectedSubcommandError::Subcommand`] when the
/// selected subcommand cannot be merged.
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_globals_and_merge_selected_subcommand<Globals, Commands, GlobalError, LoadGlobals>(
    matches: &ArgMatches,
    command: Commands,
    load_globals: LoadGlobals,
) -> Result<(Globals, Commands), LoadGlobalsAndSelectedSubcommandError<GlobalError>>
where
    Commands: SelectedSubcommandMerge,
    GlobalError: std::error::Error + Send + Sync + 'static,
    LoadGlobals: FnOnce() -> Result<Globals, GlobalError>,
{
    let globals = load_globals().map_err(LoadGlobalsAndSelectedSubcommandError::Globals)?;
    let merged = command.load_and_merge_selected(matches)?;
    Ok((globals, merged))
}
