use ortho_config::{MergeComposer, OrthoConfig};
use serde::Deserialize;
use serde_json::json;

/// Verifies that `#[ortho_config(crate = "ortho_config")]` is accepted and
/// the generated code compiles correctly. Uses the real crate name as a
/// self-referential alias so no workspace reconfiguration is needed.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(crate = "ortho_config")]
struct CratePathConfig {
    #[serde(default)]
    value: String,
    #[serde(default)]
    count: u32,
}

fn main() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({"value": "hello", "count": 1}));
    let result = CratePathConfig::merge_from_layers(composer.layers());
    let _: ortho_config::OrthoResult<CratePathConfig> = result;
}
