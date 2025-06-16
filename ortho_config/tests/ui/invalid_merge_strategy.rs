//! Test case exercising an invalid merge strategy.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
struct Bad {
    #[ortho_config(merge_strategy = "bogus")]
    values: Vec<String>,
}

fn main() {}
