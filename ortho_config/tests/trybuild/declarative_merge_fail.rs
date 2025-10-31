use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct UnsupportedStrategy {
    #[serde(default)]
    #[ortho_config(merge_strategy = "unknown")]
    values: Vec<String>,
}

fn main() {}
