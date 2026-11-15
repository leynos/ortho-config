//! Compile-time contract for source-aware generated composition delegates.

use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SOURCE_AWARE")]
struct SourceAwareConfig {
    #[serde(default)]
    value: String,
}

/// Instantiate the generated delegate to check its downstream call signature.
fn main() {
    let source = Arc::new(MapEnv::new().with_var("SOURCE_AWARE_VALUE", "injected"));
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let _composition = SourceAwareConfig::compose_layers_from_iter_with_sources(
        ["source-aware"],
        discovery,
        merge,
    );
    let _value = SourceAwareConfig {
        value: String::new(),
    }
    .value;
}
