//! Compile-fail fixture: behaviour invalid interaction

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(interaction = "sometimes"))]
struct Bad {
    value: u8,
}

fn main() {}
