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
use crate::{OrthoError, SharedScanEnvSource, subcommand::SubcommandFileContext};

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

    /// A manual implementation did not define a source-aware merge path.
    #[error("selected subcommand implementation does not support injected sources")]
    SourceInjectionUnsupported,

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

    /// Merges the selected subcommand using explicit file and environment sources.
    ///
    /// Derived implementations use `files` for named discovery lookups and
    /// `merge_source` for the subcommand environment layer. Manual implementations
    /// must override this method to support injection. The default returns
    /// [`SelectedSubcommandMergeError::SourceInjectionUnsupported`] without
    /// falling back to the process-backed method.
    ///
    /// # Errors
    ///
    /// Returns [`SelectedSubcommandMergeError::SourceInjectionUnsupported`] for
    /// a manual implementation without an injected merge path, or
    /// [`SelectedSubcommandMergeError::Merge`] when a selected file, injected
    /// environment value, or CLI value cannot be merged. Variants that
    /// require `ArgMatches` may return
    /// [`SelectedSubcommandMergeError::MissingSubcommandMatches`].
    fn load_and_merge_selected_with_sources(
        self,
        _matches: &ArgMatches,
        _files: SubcommandFileContext<'_>,
        _merge_source: SharedScanEnvSource,
    ) -> Result<Self, SelectedSubcommandMergeError> {
        Err(SelectedSubcommandMergeError::SourceInjectionUnsupported)
    }
}

/// Sources for a unified selected-subcommand load.
///
/// This context is owned by the selected-subcommand helper. It groups its two
/// distinct capabilities for that call only: `files` permits named discovery
/// lookups, while `merge_source` permits environment scanning during merging.
/// Callers should keep those capabilities separate outside the helper.
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub struct SelectedSubcommandSources<'a> {
    files: SubcommandFileContext<'a>,
    merge_source: SharedScanEnvSource,
}

#[cfg(feature = "serde_json")]
impl<'a> SelectedSubcommandSources<'a> {
    /// Groups file discovery and environment merge sources for one selected load.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::{path::Path, sync::Arc};
    /// use ortho_config::{MapEnv, SelectedSubcommandSources, SharedScanEnvSource, SubcommandFileContext};
    ///
    /// let environment = Arc::new(MapEnv::new());
    /// let files = SubcommandFileContext::new(Path::new("."), environment.as_ref());
    /// let merge: SharedScanEnvSource = environment.clone();
    /// let _sources = SelectedSubcommandSources::new(files, merge);
    /// ```
    #[must_use]
    pub fn new(files: SubcommandFileContext<'a>, merge_source: SharedScanEnvSource) -> Self {
        Self {
            files,
            merge_source,
        }
    }
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

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    //! Tests the compatibility boundary for manual selected-subcommand implementations.

    use std::{cell::Cell, path::Path, sync::Arc};

    use super::{SelectedSubcommandMerge, SelectedSubcommandMergeError};
    use crate::{MapEnv, SharedScanEnvSource, SubcommandFileContext};

    #[derive(Debug)]
    struct ManualMerge<'a> {
        legacy_called: &'a Cell<bool>,
    }

    impl SelectedSubcommandMerge for ManualMerge<'_> {
        fn load_and_merge_selected(
            self,
            _matches: &clap::ArgMatches,
        ) -> Result<Self, SelectedSubcommandMergeError> {
            self.legacy_called.set(true);
            Ok(self)
        }
    }

    #[test]
    fn manual_implementation_rejects_injected_sources_without_process_fallback() {
        let legacy_called = Cell::new(false);
        let environment = Arc::new(MapEnv::new());
        let files = SubcommandFileContext::new(Path::new("."), environment.as_ref());
        let merge_source: SharedScanEnvSource = environment.clone();

        let result = ManualMerge {
            legacy_called: &legacy_called,
        }
        .load_and_merge_selected_with_sources(
            &clap::ArgMatches::default(),
            files,
            merge_source,
        );

        assert!(
            matches!(
                result,
                Err(SelectedSubcommandMergeError::SourceInjectionUnsupported)
            ),
            "manual merge must reject unavailable source injection"
        );
        assert!(
            !legacy_called.get(),
            "source-aware merge must not invoke the process-backed legacy method"
        );
    }
}

/// Loads globals and merges the selected subcommand from explicit sources.
///
/// The globals loader remains caller-owned; the grouped sources control only
/// the selected subcommand's file discovery and environment merge layers.
/// For an injected result, use a derived `SelectedSubcommandMerge` implementation
/// or override its source-aware method in a manual implementation.
///
/// # Examples
///
/// ```rust,no_run
/// use std::{path::Path, sync::Arc};
/// use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
/// use ortho_config::{
///     MapEnv, OrthoConfig, SharedScanEnvSource,
///     SubcommandFileContext, load_globals_and_merge_selected_subcommand_with_sources,
/// };
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Parser)]
/// struct Cli { #[command(subcommand)] command: Commands }
/// #[derive(Debug, Subcommand, ortho_config_macros::SelectedSubcommandMerge)]
/// enum Commands { Run(RunArgs) }
/// #[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default)]
/// #[command(name = "run")]
/// #[ortho_config(prefix = "APP_")]
/// struct RunArgs { #[arg(long)] level: Option<u8> }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let matches = Cli::command().try_get_matches_from(["app", "run"])?;
/// let cli = Cli::from_arg_matches(&matches)?;
/// let environment = Arc::new(MapEnv::new());
/// let files = SubcommandFileContext::new(Path::new("."), environment.as_ref());
/// let merge: SharedScanEnvSource = environment.clone();
/// let sources = ortho_config::SelectedSubcommandSources::new(files, merge);
/// let (_globals, _command) = load_globals_and_merge_selected_subcommand_with_sources(
///     &matches, cli.command, sources, || Ok::<_, std::io::Error>(()),
/// )?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns [`LoadGlobalsAndSelectedSubcommandError::Globals`] if the caller's
/// loader fails, or [`LoadGlobalsAndSelectedSubcommandError::Subcommand`] if
/// the selected subcommand merge fails.
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_globals_and_merge_selected_subcommand_with_sources<
    Globals,
    Commands,
    GlobalError,
    LoadGlobals,
>(
    matches: &ArgMatches,
    command: Commands,
    sources: SelectedSubcommandSources<'_>,
    load_globals: LoadGlobals,
) -> Result<(Globals, Commands), LoadGlobalsAndSelectedSubcommandError<GlobalError>>
where
    Commands: SelectedSubcommandMerge,
    GlobalError: std::error::Error + Send + Sync + 'static,
    LoadGlobals: FnOnce() -> Result<Globals, GlobalError>,
{
    let globals = load_globals().map_err(LoadGlobalsAndSelectedSubcommandError::Globals)?;
    let merged = command.load_and_merge_selected_with_sources(
        matches,
        sources.files,
        sources.merge_source,
    )?;
    Ok((globals, merged))
}
