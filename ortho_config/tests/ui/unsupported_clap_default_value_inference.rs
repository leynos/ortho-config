//! `cli_default_as_absent` should reject inferred clap `default_value`.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, Parser, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct UnsupportedDefaultValue {
    #[arg(long, default_value = "42")]
    #[ortho_config(cli_default_as_absent)]
    answer: u32,
}

fn main() {}
