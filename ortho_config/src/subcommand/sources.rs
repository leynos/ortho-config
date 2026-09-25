//! Injected source support for subcommand configuration.
//!
//! File lookup uses named `EnvSource` access through `SubcommandFileContext`.
//! Environment merging remains the separate scanning `SharedScanEnvSource` capability.

use std::path::Path;

use super::{Prefix, load_file_and_env_defaults, load_file_and_env_defaults_at};
use crate::merge_telemetry;
use crate::{
    CliValueExtractor, CsvEnv, EnvSource, OrthoMergeExt, OrthoResult, ProcessEnv,
    SharedScanEnvSource, sanitized_provider,
};
use clap::{ArgMatches, CommandFactory};
use figment::{Figment, providers::Serialized};
use serde::de::DeserializeOwned;

/// The explicit file-search inputs for a subcommand configuration load.
///
/// This context is limited to named environment lookups for subcommand file
/// candidates. It cannot scan the environment and must not be reused as a
/// general environment service.
#[derive(Debug, Clone, Copy)]
pub struct SubcommandFileContext<'a> {
    pub(super) base: &'a Path,
    pub(super) discovery: &'a dyn EnvSource,
}

impl<'a> SubcommandFileContext<'a> {
    /// Creates a file-search context rooted at `base`.
    #[must_use]
    pub fn new(base: &'a Path, discovery: &'a dyn EnvSource) -> Self {
        Self { base, discovery }
    }
}

/// Parsed CLI values paired with their clap match metadata.
#[derive(Debug, Clone, Copy)]
pub struct SubcommandCliMatches<'a, T> {
    pub(super) cli: &'a T,
    pub(super) matches: &'a ArgMatches,
}

impl<'a, T> SubcommandCliMatches<'a, T> {
    /// Pairs parsed CLI values with the matches that identify explicit values.
    #[must_use]
    pub const fn new(cli: &'a T, matches: &'a ArgMatches) -> Self {
        Self { cli, matches }
    }
}

/// Build the injected provider used by the subcommand environment layer.
pub(super) fn env_provider(prefix: &str, source: SharedScanEnvSource) -> CsvEnv {
    CsvEnv::prefixed(prefix)
        .csv(false)
        .split("__")
        .with_source(source)
}

/// Loads defaults using explicit file and merge sources, then overlays CLI values.
///
/// # Usage
///
/// Pass a [`SubcommandFileContext`] rooted at the intended file base and a
/// scanning merge source. A successful result combines those defaults with CLI
/// values without reading the process environment.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if a selected file, injected environment, or
/// CLI value cannot be merged into `T`.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_with_sources_at<T>(
    prefix: &Prefix,
    cli: &T,
    files: SubcommandFileContext<'_>,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    merge_telemetry::source_aware_subcommand_load_started();
    let result = (|| {
        let fig = load_file_and_env_defaults_at::<T>(prefix, files, Some(merge_source))?;
        fig.merge(sanitized_provider(cli)?)
            .extract()
            .into_ortho_merge()
    })();
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
}

/// Loads defaults using the configured prefix and explicit file and merge sources.
///
/// # Usage
///
/// Supply the type's parsed CLI values, an explicit file context, and a scan
/// source. The result uses `T::prefix()` while retaining the same precedence.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if a selected file, injected environment, or
/// CLI value cannot be merged into `T`.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_sources_at<T>(
    cli: &T,
    files: SubcommandFileContext<'_>,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    load_and_merge_subcommand_with_sources_at(&Prefix::new(T::prefix()), cli, files, merge_source)
}

/// Loads defaults using explicit sources and preserves `cli_default_as_absent` values.
///
/// # Usage
///
/// Pair parsed CLI values with their matches and pass a context plus scan
/// source. The result keeps CLI defaults absent unless clap marked them explicit.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if a selected file, injected environment, or
/// CLI value cannot be merged into `T`.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_with_matches_with_sources_at<T>(
    prefix: &Prefix,
    cli_matches: &SubcommandCliMatches<'_, T>,
    files: SubcommandFileContext<'_>,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory + CliValueExtractor,
{
    merge_telemetry::source_aware_subcommand_load_started();
    let result = (|| {
        let fig = Figment::from(Serialized::defaults(T::default())).merge(
            load_file_and_env_defaults_at::<T>(prefix, files, Some(merge_source))?,
        );
        let cli_value = cli_matches.cli.extract_user_provided(cli_matches.matches)?;
        fig.merge(Serialized::defaults(cli_value))
            .extract()
            .into_ortho_merge()
    })();
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
}

/// Loads configured-prefix defaults using explicit sources and match metadata.
///
/// # Usage
///
/// Use this wrapper when `T` supplies its prefix. Its result retains only clap
/// values marked explicit while using the supplied file and scan capabilities.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if a selected file, injected environment, or
/// CLI value cannot be merged into `T`.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_matches_with_sources_at<T>(
    cli_matches: &SubcommandCliMatches<'_, T>,
    files: SubcommandFileContext<'_>,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory + CliValueExtractor,
{
    load_and_merge_subcommand_with_matches_with_sources_at(
        &Prefix::new(T::prefix()),
        cli_matches,
        files,
        merge_source,
    )
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
    let process_env = ProcessEnv;
    load_and_merge_subcommand_with_sources_at(
        prefix,
        cli,
        SubcommandFileContext::new(Path::new("."), &process_env),
        merge_source,
    )
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
    merge_telemetry::source_aware_subcommand_load_started();
    let result = (|| {
        let fig = Figment::from(Serialized::defaults(T::default()))
            .merge(load_file_and_env_defaults::<T>(prefix, Some(merge_source))?);
        let cli_value = cli.extract_user_provided(matches)?;
        fig.merge(Serialized::defaults(cli_value))
            .extract()
            .into_ortho_merge()
    })();
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
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
