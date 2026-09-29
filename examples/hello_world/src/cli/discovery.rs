//! Configuration discovery helpers for the `hello_world` CLI.
//!
//! The internal [`discovery`] function constructs the shared
//! [`ortho_config::ConfigDiscovery`] instance used across the example so all
//! entrypoints observe the same search order. Test builds on Unix platforms
//! use [`collect_config_candidates`] to inspect UTF-8 candidate paths
//! directly. Production code calls [`discover_config_layer`] to load the
//! first readable configuration file. The `cfg(all(test, unix))` guard keeps
//! the test helper out of non-Unix builds to avoid dead-code warnings while
//! documenting its availability for behavioural coverage.

#[cfg(all(test, unix))]
use camino::Utf8PathBuf;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ortho_config::{SharedEnvSource, process_env_source};

use crate::error::HelloWorldError;

/// Build discovery using the process environment for the default CLI path.
///
/// For example, the ordinary command-line loader uses this to retain its
/// existing environment-variable and filesystem candidate search.
fn discovery() -> ortho_config::ConfigDiscovery {
    discovery_with_source(process_env_source())
}

/// Build discovery with an explicit lookup source and ambient default roots.
///
/// For example, standalone source-aware override loading uses injected
/// environment lookups while preserving the usual project and current roots.
fn discovery_with_source(source: SharedEnvSource) -> ortho_config::ConfigDiscovery {
    discovery_builder(source).build()
}

/// Build discovery with an explicit lookup source and project root.
///
/// For example, greeting defaults use their supplied fixture directory as the
/// only root when looking up a configuration file.
pub(super) fn discovery_with_source_at(
    source: SharedEnvSource,
    project_root: &Path,
) -> ortho_config::ConfigDiscovery {
    discovery_builder(source)
        .project_roots([PathBuf::from(project_root)])
        .build()
}

/// Configure the example's environment selectors and candidate filenames.
///
/// For example, production and fixture discovery share the same names while
/// choosing different roots and environment sources.
fn discovery_builder(source: SharedEnvSource) -> ortho_config::discovery::ConfigDiscoveryBuilder {
    ortho_config::ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .config_file_name("hello_world.toml")
        .dotfile_name(".hello_world.toml")
        .project_file_name(".hello_world.toml")
        .env_source(source)
}

#[cfg(all(test, unix))]
pub(super) fn collect_config_candidates() -> Vec<Utf8PathBuf> {
    discovery().utf8_candidates()
}

/// Discover the first configuration layer using the process-backed defaults.
///
/// For example, this is the file layer used by ordinary global configuration
/// loading.
pub(super) fn discover_config_layer()
-> Result<Option<ortho_config::MergeLayer<'static>>, HelloWorldError> {
    discover_config_layer_from(&discovery())
}

/// Discover and load the first configuration layer using an injected source.
///
/// For example, tests can select a config path from a closed environment map.
pub(super) fn discover_config_layer_with_source(
    source: SharedEnvSource,
) -> Result<Option<ortho_config::MergeLayer<'static>>, HelloWorldError> {
    discover_config_layer_from(&discovery_with_source(source))
}

/// Convert a discovery outcome into an optional merge layer or an error.
///
/// For example, no matching candidate returns `Ok(None)`, while a readable
/// candidate returns its layer.
pub(super) fn discover_config_layer_from(
    discovery: &ortho_config::ConfigDiscovery,
) -> Result<Option<ortho_config::MergeLayer<'static>>, HelloWorldError> {
    let mut outcome = discovery.compose_layer();
    if let Some(layer) = outcome.value {
        return Ok(Some(layer));
    }

    outcome.required_errors.append(&mut outcome.optional_errors);
    ortho_config::OrthoError::try_aggregate(outcome.required_errors).map_or_else(
        || Ok(None),
        |err| Err(HelloWorldError::Configuration(Arc::new(err))),
    )
}
