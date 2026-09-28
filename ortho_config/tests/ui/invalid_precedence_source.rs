//! Test case exercising an invalid precedence source spelling.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(precedence(order = ["defaults", "file", "bogus", "cli"]))]
struct Bad {
    #[serde(default)]
    retries: u32,
}

fn main() {}
