//! Compile-fail fixture: behaviour unknown nested key

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(interation = "interactive"))]
struct Bad {
    value: u8,
}

fn main() {}
