//! Compile-fail fixture: behaviour noninteractive bypass

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(
    interaction = "non_interactive",
    bypass = "--force"
))]
struct Bad {
    value: u8,
}

fn main() {}
