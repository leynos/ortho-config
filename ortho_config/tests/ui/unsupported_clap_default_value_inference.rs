//! `cli_default_as_absent` should reject unsupported string default shapes.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, Parser, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct UnsupportedDefaultValueShape {
    #[arg(long, default_value = "value")]
    #[ortho_config(cli_default_as_absent)]
    values: Option<Vec<String>>,
}

fn main() {}
