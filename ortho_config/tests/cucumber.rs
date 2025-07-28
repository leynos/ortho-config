use clap::Parser;
use cucumber::World as _;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, cucumber::World)]
pub struct World {
    env_value: Option<String>,
    pub result: Option<Result<RulesConfig, ortho_config::OrthoError>>,
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct RulesConfig {
    #[arg(long)]
    rules: Vec<String>,
}

mod steps;

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
