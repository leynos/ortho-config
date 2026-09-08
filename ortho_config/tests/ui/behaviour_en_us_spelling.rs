//! Compile-fail fixture: behaviour en us spelling

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behavior(interaction = "interactive"))]
struct Bad {
    value: u8,
}

fn main() {}
