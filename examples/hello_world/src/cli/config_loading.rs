//! Helpers that expose configuration file overrides for subcommands.

use std::sync::Arc;

use camino::Utf8PathBuf;
use ortho_config::{ConfigDiscovery, MergeLayer, SharedEnvSource};

use crate::error::HelloWorldError;

use super::{
    discovery::{
        discover_config_layer, discover_config_layer_from, discover_config_layer_with_source,
    },
    overrides::FileOverrides,
};

pub(crate) fn load_config_overrides_with_layer(
    candidate: Option<MergeLayer<'static>>,
) -> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let Some(layer) = candidate else {
        return Ok(None);
    };

    let path = layer.path().map(camino::Utf8PathBuf::from);
    let value = layer.into_value();
    let overrides: FileOverrides = ortho_config::serde_json::from_value(value)
        .map_err(|err| HelloWorldError::Configuration(Arc::new(err.into())))?;
    Ok(Some((overrides, path)))
}

pub(crate) fn load_config_overrides()
-> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer()?;
    load_config_overrides_with_layer(layer)
}

/// Load file overrides using an injected source and the default candidate roots.
///
/// For example, standalone greeting override loading can use a test map for
/// `HELLO_WORLD_CONFIG_PATH` while retaining ordinary candidate discovery.
pub(crate) fn load_config_overrides_with_source(
    discovery: SharedEnvSource,
) -> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer_with_source(discovery)?;
    load_config_overrides_with_layer(layer)
}

/// Load file overrides from an already configured discovery context.
///
/// For example, callers can provide explicit fixture roots to keep discovery
/// independent of the process working directory.
pub(crate) fn load_config_overrides_from_discovery(
    discovery: &ConfigDiscovery,
) -> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer_from(discovery)?;
    load_config_overrides_with_layer(layer)
}
