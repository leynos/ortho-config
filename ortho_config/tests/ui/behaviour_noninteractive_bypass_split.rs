//! Compile-fail fixture: behaviour noninteractive bypass split across groups

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(interaction = "non_interactive"))]
#[ortho_config(behaviour(bypass = "--force"))]
struct Bad {
    value: u8,
}

fn main() {}
