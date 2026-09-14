use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Parser, OrthoConfig)]
#[ortho_config(prefix = "APP_", profiles)]
struct Collides {
    #[arg(env = "APP_PROFILE")]
    thing: Option<String>,
}

fn main() {}