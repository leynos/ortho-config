use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles)]
struct Collides {
    #[ortho_config(cli_long = "profile")]
    thing: Option<String>,
}

fn main() {}