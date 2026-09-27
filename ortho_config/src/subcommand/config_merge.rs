//! The convenience merge trait for subcommand configuration structs.
//!
//! [`SubcmdConfigMerge`] gives every applicable subcommand struct a
//! `load_and_merge` family of methods, so an application does not repeat an
//! identical inherent method on each subcommand variant.
//!
//! Each method forwards to the corresponding `load_and_merge_subcommand_for`
//! helper. The trait adds no behaviour of its own; it exists so call sites can
//! write `cli.load_and_merge()?` instead of naming the helper and the prefix.

use clap::{ArgMatches, CommandFactory};

use super::{
    load_and_merge_subcommand_for, load_and_merge_subcommand_for_with_matches,
    load_and_merge_subcommand_for_with_matches_with_sources,
    load_and_merge_subcommand_for_with_sources,
};
use crate::{CliValueExtractor, OrthoConfig, OrthoResult, SharedScanEnvSource};

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
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub trait SubcmdConfigMerge: OrthoConfig + CommandFactory + Sized {
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

    /// Merge configuration defaults using an injected environment merge source.
    ///
    /// # Errors
    ///
    /// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or
    /// the merged defaults cannot be deserialised.
    fn load_and_merge_with_sources(&self, merge_source: SharedScanEnvSource) -> OrthoResult<Self>
    where
        Self: serde::Serialize + Default,
    {
        load_and_merge_subcommand_for_with_sources(self, merge_source)
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

    /// Merge configuration defaults from an injected source, respecting CLI
    /// default values marked as absent.
    ///
    /// # Errors
    ///
    /// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or
    /// the merged defaults cannot be deserialised.
    fn load_and_merge_with_matches_with_sources(
        &self,
        matches: &ArgMatches,
        merge_source: SharedScanEnvSource,
    ) -> OrthoResult<Self>
    where
        Self: serde::Serialize + Default + CliValueExtractor,
    {
        load_and_merge_subcommand_for_with_matches_with_sources(self, matches, merge_source)
    }
}

impl<T> SubcmdConfigMerge for T where T: OrthoConfig + serde::Serialize + Default + CommandFactory {}
