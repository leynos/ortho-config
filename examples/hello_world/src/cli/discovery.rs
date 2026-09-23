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

use std::sync::Arc;

use ortho_config::{SharedEnvSource, process_env_source};

use crate::error::HelloWorldError;

fn discovery() -> ortho_config::ConfigDiscovery {
    discovery_with_source(process_env_source())
}

fn discovery_with_source(source: SharedEnvSource) -> ortho_config::ConfigDiscovery {
    ortho_config::ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .config_file_name("hello_world.toml")
        .dotfile_name(".hello_world.toml")
        .project_file_name(".hello_world.toml")
        .env_source(source)
        .build()
}

#[cfg(all(test, unix))]
pub(super) fn collect_config_candidates() -> Vec<Utf8PathBuf> {
    discovery().utf8_candidates()
}

pub(super) fn discover_config_layer()
-> Result<Option<ortho_config::MergeLayer<'static>>, HelloWorldError> {
    discover_config_layer_from(&discovery())
}

pub(super) fn discover_config_layer_with_source(
    source: SharedEnvSource,
) -> Result<Option<ortho_config::MergeLayer<'static>>, HelloWorldError> {
    discover_config_layer_from(&discovery_with_source(source))
}

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
