//! Helpers that expose configuration file overrides for subcommands.

use std::sync::Arc;

use camino::Utf8PathBuf;
#[cfg(test)]
use ortho_config::ConfigDiscovery;
use ortho_config::{MergeLayer, SharedEnvSource};

use crate::error::HelloWorldError;

#[cfg(test)]
use super::discovery::discover_config_layer_from;
use super::{
    discovery::{discover_config_layer, discover_config_layer_with_source},
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

pub(crate) fn load_config_overrides_with_source(
    discovery: SharedEnvSource,
) -> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer_with_source(discovery)?;
    load_config_overrides_with_layer(layer)
}

#[cfg(test)]
pub(crate) fn load_config_overrides_from_discovery(
    discovery: &ConfigDiscovery,
) -> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer_from(discovery)?;
    load_config_overrides_with_layer(layer)
}
