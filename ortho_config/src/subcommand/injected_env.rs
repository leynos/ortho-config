//! Single-environment entry points for subcommand configuration.
//!
//! One injected environment backs both halves of a subcommand load: the named
//! lookups that locate files and the prefix scan that supplies the environment
//! layer. The working directory comes from the same object
//! ([`EnvSource::current_dir`]), so a test that pins it with
//! [`crate::MapEnv::with_current_dir`] reads and mutates nothing process-wide.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{ArgMatches, CommandFactory};

use super::{
    SubcommandCliMatches, SubcommandFileContext,
    load_and_merge_subcommand_for_with_matches_with_sources_at,
    load_and_merge_subcommand_for_with_sources_at,
};
use crate::{
    CliValueExtractor, EnvSource, OrthoConfig, OrthoError, OrthoResult, RequiredConfigSchema,
    ScanEnvSource,
};

/// Base directory for file candidates: the environment's working directory.
///
/// A failed lookup is an error, not a fallback to `.`: a process-relative
/// candidate would read the very directory the injected source exists to keep
/// out of the load.
///
/// # Errors
///
/// Returns [`OrthoError::Validation`] when the source cannot report its
/// working directory.
fn file_base(env: &dyn EnvSource) -> OrthoResult<PathBuf> {
    env.current_dir().map_err(|error| {
        Arc::new(OrthoError::Validation {
            key: "current_dir".to_owned(),
            message: format!("the injected environment has no working directory: {error}"),
        })
    })
}

/// Merge subcommand defaults from one injected environment, then overlay CLI values.
///
/// The schema-aware entry point is deliberate: `T` carries a derived
/// [`RequiredConfigSchema`], so absent required values are enumerated rather
/// than left to the deserializer.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if the environment has no working directory,
/// [`crate::OrthoError::MissingRequiredValues`] if a required value is absent,
/// or another [`crate::OrthoError`] if a selected file, the injected
/// environment, or a CLI value cannot be merged into `T`.
pub(super) fn load_and_merge_for_with_env<T, E>(cli: &T, env: &Arc<E>) -> OrthoResult<T>
where
    T: OrthoConfig + serde::Serialize + Default + CommandFactory + RequiredConfigSchema,
    E: EnvSource + ScanEnvSource + 'static,
{
    let base = file_base(env.as_ref())?;
    load_and_merge_subcommand_for_with_sources_at(
        cli,
        SubcommandFileContext::new(&base, env.as_ref()),
        env.clone(),
    )
}

/// As [`load_and_merge_for_with_env`], keeping `cli_default_as_absent` values absent.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if the environment has no working directory,
/// [`crate::OrthoError::MissingRequiredValues`] if a required value is absent,
/// or another [`crate::OrthoError`] if a selected file, the injected
/// environment, or a CLI value cannot be merged into `T`.
pub(super) fn load_and_merge_for_with_matches_with_env<T, E>(
    cli: &T,
    matches: &ArgMatches,
    env: &Arc<E>,
) -> OrthoResult<T>
where
    T: OrthoConfig
        + serde::Serialize
        + Default
        + CommandFactory
        + CliValueExtractor
        + RequiredConfigSchema,
    E: EnvSource + ScanEnvSource + 'static,
{
    let base = file_base(env.as_ref())?;
    load_and_merge_subcommand_for_with_matches_with_sources_at(
        &SubcommandCliMatches::new(cli, matches),
        SubcommandFileContext::new(&base, env.as_ref()),
        env.clone(),
    )
}
