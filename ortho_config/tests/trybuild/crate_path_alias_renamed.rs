//! Trybuild fixture verifying `#[ortho_config(crate = "...")]` works
//! with a genuine dependency rename via `use ... as`.

use ortho_config as my_cfg;
use my_cfg::{MergeComposer, OrthoConfig};
use serde::Deserialize;
use serde_json::json;

/// Verifies that `#[ortho_config(crate = "my_cfg")]` generates code that
/// references types through the aliased name rather than `ortho_config`.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(crate = "my_cfg")]
struct AliasedConfig {
    #[serde(default)]
    value: String,
    #[serde(default)]
    count: u32,
}

fn main() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({"value": "hello", "count": 1}));
    let result = AliasedConfig::merge_from_layers(composer.layers());
    let _: my_cfg::OrthoResult<AliasedConfig> = result;
}
