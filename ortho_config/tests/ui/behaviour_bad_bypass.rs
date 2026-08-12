//! Compile-fail fixture: behaviour bad bypass

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(bypass = "force"))]
struct Bad {
    value: u8,
}

fn main() {}
