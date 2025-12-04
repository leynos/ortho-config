//! Helpers that expose configuration file overrides for subcommands.

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
    let overrides =
        ortho_config::declarative::from_value(value).map_err(HelloWorldError::Configuration)?;
    Ok(Some((overrides, path)))
}

pub(crate) fn load_config_overrides()
-> Result<Option<(FileOverrides, Option<Utf8PathBuf>)>, HelloWorldError> {
    let layer = discover_config_layer()?;
    load_config_overrides_with_layer(layer)
}
