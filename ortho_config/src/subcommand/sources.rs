//! Injected environment-source support for subcommand configuration.
//!
//! The process-backed provider remains in the parent module. These entry
//! points select `CsvEnv` only when callers supply a scanning source.

use super::{Prefix, load_file_and_env_defaults};
use crate::{
    CliValueExtractor, CsvEnv, OrthoMergeExt, OrthoResult, SharedScanEnvSource, sanitized_provider,
};
use clap::{ArgMatches, CommandFactory};
use figment::{Figment, providers::Serialized};
use serde::de::DeserializeOwned;

/// Build the injected provider used by the subcommand environment layer.
pub(super) fn env_provider(prefix: &str, source: SharedScanEnvSource) -> CsvEnv {
    CsvEnv::prefixed(prefix)
        .csv(false)
        .split("__")
        .with_source(source)
}

/// Loads defaults with an injected environment merge source, then overlays CLI
/// values.
///
/// The source replaces only the subcommand environment layer. File discovery
/// retains its existing behaviour.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or the
/// merged defaults cannot be deserialised.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_with_sources<T>(
    prefix: &Prefix,
    cli: &T,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    let fig = load_file_and_env_defaults::<T>(prefix, Some(merge_source))?;
    fig.merge(sanitized_provider(cli)?)
        .extract()
        .into_ortho_merge()
}

/// Wrapper around [`load_and_merge_subcommand_with_sources`] using the
/// struct's configured prefix.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or the
/// merged defaults cannot be deserialised.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_sources<T>(
    cli: &T,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    load_and_merge_subcommand_with_sources(&Prefix::new(T::prefix()), cli, merge_source)
}

/// Loads defaults from files and an injected merge source, respecting fields
/// marked `cli_default_as_absent`.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or the
/// merged defaults cannot be deserialised.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_with_matches_with_sources<T>(
    prefix: &Prefix,
    cli: &T,
    matches: &ArgMatches,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory + CliValueExtractor,
{
    let fig = Figment::from(Serialized::defaults(T::default()))
        .merge(load_file_and_env_defaults::<T>(prefix, Some(merge_source))?);
    let cli_value = cli.extract_user_provided(matches)?;
    fig.merge(Serialized::defaults(cli_value))
        .extract()
        .into_ortho_merge()
}

/// Wrapper around [`load_and_merge_subcommand_with_matches_with_sources`]
/// using the struct's configured prefix.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or the
/// merged defaults cannot be deserialised.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_matches_with_sources<T>(
    cli: &T,
    matches: &ArgMatches,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory + CliValueExtractor,
{
    load_and_merge_subcommand_with_matches_with_sources(
        &Prefix::new(T::prefix()),
        cli,
        matches,
        merge_source,
    )
}
