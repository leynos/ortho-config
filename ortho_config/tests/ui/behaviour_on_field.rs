//! Compile-fail fixture: behaviour on field

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(prefix = "X")]
struct Bad {
    #[ortho_config(behaviour(mutation = "write"))]
    value: u8,
}

fn main() {}
