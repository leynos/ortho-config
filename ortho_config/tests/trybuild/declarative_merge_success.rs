use std::collections::BTreeMap;

use ortho_config::{MergeComposer, OrthoConfig};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, OrthoConfig)]
struct DeclarativeTrybuildConfig {
    #[serde(default)]
    append: Vec<String>,
    #[serde(default)]
    #[ortho_config(merge_strategy = "replace")]
    replace: Vec<String>,
    #[serde(default)]
    #[ortho_config(skip_cli)]
    maps: BTreeMap<String, String>,
    #[serde(default)]
    #[ortho_config(merge_strategy = "replace")]
    #[ortho_config(skip_cli)]
    keyed_replace: BTreeMap<String, String>,
}

fn main() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "append": ["defaults"],
        "replace": ["defaults"],
        "maps": {"defaults": "value"},
        "keyed_replace": {"defaults": "value"}
    }));
    composer.push_file(json!({
        "append": ["file"],
        "replace": ["file"],
        "maps": {"file": "value"},
        "keyed_replace": {"file": "value"}
    }), None);
    composer.push_environment(json!({
        "append": ["env"],
        "replace": ["env"],
        "maps": {"env": "value"},
        "keyed_replace": {"env": "value"}
    }));
    composer.push_cli(json!({
        "append": ["cli"],
        "replace": ["cli"],
        "maps": {"cli": "value"},
        "keyed_replace": {"cli": "value"}
    }));

    let result = DeclarativeTrybuildConfig::merge_from_layers(composer.layers());
    let _: ortho_config::OrthoResult<DeclarativeTrybuildConfig> = result;
}
