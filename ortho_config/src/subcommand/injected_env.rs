//! Single-environment entry points for subcommand configuration.
//!
//! One injected environment backs both halves of a subcommand load: the named
//! lookups that locate files and the prefix scan that supplies the environment
//! layer. The working directory comes from the same object
//! ([`EnvSource::current_dir`]), so a test that pins it with
//! [`crate::MapEnv::with_current_dir`] reads and mutates nothing process-wide.

use std::path::PathBuf;
use std::sync::Arc;

use clap::CommandFactory;
use serde::de::DeserializeOwned;

use super::{
    Prefix, SubcommandCliMatches, SubcommandFileContext,
    load_and_merge_subcommand_with_matches_with_sources_at,
    load_and_merge_subcommand_with_sources_at,
};
use crate::{CliValueExtractor, EnvSource, OrthoConfig, OrthoResult, ScanEnvSource};
use clap::ArgMatches;

/// Base directory for file candidates: the environment's working directory.
///
/// An unreadable working directory falls back to `.`, matching the
/// process-backed loaders, which always resolved candidates against `.`.
fn file_base(env: &dyn EnvSource) -> PathBuf {
    env.current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Merge subcommand defaults from one injected environment, then overlay CLI values.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if a selected file, the injected environment,
/// or a CLI value cannot be merged into `T`.
pub(super) fn load_and_merge_for_with_env<T, E>(cli: &T, env: &Arc<E>) -> OrthoResult<T>
where
    T: OrthoConfig + serde::Serialize + DeserializeOwned + Default + CommandFactory,
    E: EnvSource + ScanEnvSource + 'static,
{
    let base = file_base(env.as_ref());
    load_and_merge_subcommand_with_sources_at(
        &Prefix::new(T::prefix()),
        cli,
        SubcommandFileContext::new(&base, env.as_ref()),
        env.clone(),
    )
}

/// As [`load_and_merge_for_with_env`], keeping `cli_default_as_absent` values absent.
///
/// # Errors
///
/// Returns [`crate::OrthoError`] if a selected file, the injected environment,
/// or a CLI value cannot be merged into `T`.
pub(super) fn load_and_merge_for_with_matches_with_env<T, E>(
    cli: &T,
    matches: &ArgMatches,
    env: &Arc<E>,
) -> OrthoResult<T>
where
    T: OrthoConfig
        + serde::Serialize
        + DeserializeOwned
        + Default
        + CommandFactory
        + CliValueExtractor,
    E: EnvSource + ScanEnvSource + 'static,
{
    let base = file_base(env.as_ref());
    load_and_merge_subcommand_with_matches_with_sources_at(
        &Prefix::new(T::prefix()),
        &SubcommandCliMatches::new(cli, matches),
        SubcommandFileContext::new(&base, env.as_ref()),
        env.clone(),
    )
}
