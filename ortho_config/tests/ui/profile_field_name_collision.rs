use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles)]
struct Collides {
    profile: Option<String>,
}

fn main() {}