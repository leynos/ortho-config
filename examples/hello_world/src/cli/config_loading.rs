//! Helpers that expose configuration file overrides for subcommands.

use std::sync::Arc;

use camino::Utf8PathBuf;
use ortho_config::MergeLayer;

use crate::error::HelloWorldError;

use super::{discovery::discover_config_layer, overrides::FileOverrides};

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
