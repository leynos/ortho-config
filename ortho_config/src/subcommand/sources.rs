//! Injected source support for subcommand configuration.
//!
//! File lookup uses named `EnvSource` access through `SubcommandFileContext`.
//! Environment merging remains the separate scanning `SharedScanEnvSource` capability.

use std::path::Path;

use super::{CmdName, Prefix, load_file_and_env_defaults_at, subcommand_env_prefix};
use crate::merge_telemetry;
use crate::required_values::RouteScope;
use crate::{
    CliValueExtractor, CsvEnv, EnvSource, OrthoMergeExt, OrthoResult, ProcessEnv,
    RequiredConfigSchema, SharedScanEnvSource, sanitized_provider,
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
    /// Base directory used to resolve file candidates.
    pub(super) base: &'a Path,
    /// Named lookups for discovery values; this capability does not enumerate
    /// environment keys.
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
    /// Parsed values merged after defaults and selected file layers.
    pub(super) cli: &'a T,
    /// Clap's match tree, used to distinguish explicit values from defaults.
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

/// Build the merged figment for an injected subcommand load.
///
/// Kept separate from the finishing step so the schema-aware `_for` entry
/// points reuse exactly the same layer construction as the raw helpers.
fn injected_figment<T>(
    prefix: &Prefix,
    cli: &T,
    files: SubcommandFileContext<'_>,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<Figment>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    let fig = load_file_and_env_defaults_at::<T>(prefix, files, Some(merge_source))?;
    Ok(fig.merge(sanitized_provider(cli)?))
}

/// Build the merged figment for an injected matches-aware subcommand load.
fn injected_matches_figment<T>(
    prefix: &Prefix,
    cli_matches: &SubcommandCliMatches<'_, T>,
    files: SubcommandFileContext<'_>,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<Figment>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory + CliValueExtractor,
{
    let fig = Figment::from(Serialized::defaults(T::default())).merge(
        load_file_and_env_defaults_at::<T>(prefix, files, Some(merge_source))?,
    );
    let cli_value = cli_matches.cli.extract_user_provided(cli_matches.matches)?;
    Ok(fig.merge(Serialized::defaults(cli_value)))
}

/// Finish a subcommand load, enumerating absent required values.
///
/// Only the `_for` entry points call this, because only they carry a derived
/// [`RequiredConfigSchema`]. The raw helpers keep their `DeserializeOwned`
/// boundary: their callers may name a type with no schema metadata, and
/// claiming otherwise would be a false promise.
///
/// The report is labelled with the subcommand's clap name, which is what a
/// user typed on the command line, and its routes are scoped to the namespaces
/// this loader actually reads: `PREFIX_CMDS_<NAME>_*` variables and the
/// `[cmds.<name>]` file section.
pub(super) fn finish_with_required_values<T>(fig: &Figment, prefix: &Prefix) -> OrthoResult<T>
where
    T: DeserializeOwned + CommandFactory + RequiredConfigSchema,
{
    let mut value: serde_json::Value = fig.extract().into_ortho_merge()?;
    // The derived finish path normalises an empty accumulator the same way: a
    // layer set that supplied nothing extracts as `Null`, which is not a JSON
    // object and would otherwise skip enumeration silently.
    if value.is_null() {
        value = serde_json::Value::Object(serde_json::Map::new());
    }
    let name = CmdName::new(T::command().get_name());
    let scope = RouteScope::subcommand(subcommand_env_prefix(prefix, &name), name.as_str());
    crate::required_values::ensure_no_missing_required_values_scoped::<T>(
        &value,
        name.as_str(),
        &scope,
    )?;
    crate::declarative::from_value_merge(value)
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
    let result = injected_figment(prefix, cli, files, merge_source)
        .and_then(|fig| fig.extract().into_ortho_merge());
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
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory + RequiredConfigSchema,
{
    let prefix = Prefix::new(T::prefix());
    merge_telemetry::source_aware_subcommand_load_started();
    let result = injected_figment(&prefix, cli, files, merge_source)
        .and_then(|fig| finish_with_required_values(&fig, &prefix));
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
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
    let result = injected_matches_figment(prefix, cli_matches, files, merge_source)
        .and_then(|fig| fig.extract().into_ortho_merge());
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
    T: crate::OrthoConfig
        + serde::Serialize
        + Default
        + CommandFactory
        + CliValueExtractor
        + RequiredConfigSchema,
{
    let prefix = Prefix::new(T::prefix());
    merge_telemetry::source_aware_subcommand_load_started();
    let result = injected_matches_figment(&prefix, cli_matches, files, merge_source)
        .and_then(|fig| finish_with_required_values(&fig, &prefix));
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
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
/// merged defaults cannot be deserialized.
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
/// merged defaults cannot be deserialized.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_sources<T>(
    cli: &T,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory + RequiredConfigSchema,
{
    let process_env = ProcessEnv;
    let prefix = Prefix::new(T::prefix());
    merge_telemetry::source_aware_subcommand_load_started();
    let result = injected_figment(
        &prefix,
        cli,
        SubcommandFileContext::new(Path::new("."), &process_env),
        merge_source,
    )
    .and_then(|fig| finish_with_required_values(&fig, &prefix));
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
}

/// Loads defaults from files and an injected merge source, respecting fields
/// marked `cli_default_as_absent`.
///
/// File discovery keeps its established process-backed behaviour: the local
/// base is the working directory and names are resolved through [`ProcessEnv`].
/// Use [`load_and_merge_subcommand_with_matches_with_sources_at`] when the file
/// search itself must be injected.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or the
/// merged defaults cannot be deserialized.
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
    let process_env = ProcessEnv;
    load_and_merge_subcommand_with_matches_with_sources_at(
        prefix,
        &SubcommandCliMatches::new(cli, matches),
        SubcommandFileContext::new(Path::new("."), &process_env),
        merge_source,
    )
}

/// Wrapper around [`load_and_merge_subcommand_with_matches_with_sources`]
/// using the struct's configured prefix.
///
/// # Errors
///
/// Returns [`crate::OrthoError::Merge`] if CLI values cannot be merged or the
/// merged defaults cannot be deserialized.
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn load_and_merge_subcommand_for_with_matches_with_sources<T>(
    cli: &T,
    matches: &ArgMatches,
    merge_source: SharedScanEnvSource,
) -> OrthoResult<T>
where
    T: crate::OrthoConfig
        + serde::Serialize
        + Default
        + CommandFactory
        + CliValueExtractor
        + RequiredConfigSchema,
{
    let process_env = ProcessEnv;
    let prefix = Prefix::new(T::prefix());
    merge_telemetry::source_aware_subcommand_load_started();
    let result = injected_matches_figment(
        &prefix,
        &SubcommandCliMatches::new(cli, matches),
        SubcommandFileContext::new(Path::new("."), &process_env),
        merge_source,
    )
    .and_then(|fig| finish_with_required_values(&fig, &prefix));
    merge_telemetry::source_aware_subcommand_load_finished(&result);
    result
}
