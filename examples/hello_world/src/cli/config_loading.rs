//! Helpers that expose configuration file overrides for subcommands.

use std::sync::Arc;

use camino::Utf8PathBuf;
use ortho_config::MergeLayer;

use crate::error::HelloWorldError;

use super::{discovery::discover_config_layer, overrides::FileOverrides};

/// Decodes a discovered merge layer into file overrides and its source path.
///
/// An absent layer stays `None`, while a present layer is consumed so its
/// sanitized value can be deserialized without retaining provider state.
///
/// # Errors
///
/// Returns a configuration error when the layer does not match the override
/// schema.
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

/// Discovers the first configuration layer and decodes its override document.
///
/// # Errors
///
/// Returns a configuration error when discovery or deserialization fails.
pub(crate) fn load_config_overrides()
-> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer()?;
    load_config_overrides_with_layer(layer)
}
