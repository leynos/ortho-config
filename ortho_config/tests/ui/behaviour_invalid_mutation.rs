//! Compile-fail fixture: behaviour invalid mutation

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(mutation = "destroy"))]
struct Bad {
    value: u8,
}

fn main() {}
