//! Tests for attribute handling in the derive macro.

#![allow(non_snake_case)]

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
#[allow(dead_code)]
struct CustomCli {
    #[ortho_config(cli_long = "my-val")]
    #[arg(long = "my-val")]
    value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
#[allow(dead_code)]
#[ortho_config(prefix = "CFG_")]
struct Prefixed {
    #[arg(long)]
    value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
#[allow(dead_code)]
struct Defaulted {
    #[ortho_config(default = 5)]
    #[serde(skip_serializing_if = "Option::is_none")]
    num: Option<u32>,
}

#[test]
fn uses_custom_cli_long() {
    let cli = CustomCli::parse_from(["prog", "--my-val", "42"]);
    assert_eq!(cli.value.as_deref(), Some("42"));
}

#[test]
fn env_prefix_is_used() {
    figment::Jail::expect_with(|j| {
        j.set_env("CFG_VALUE", "env");
        let cli = Prefixed::parse_from(["prog", "--value", "cli"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.value.as_deref(), Some("cli"));
        Ok(())
    });
}

#[test]
fn default_value_applied() {
    let cli = Defaulted::parse_from(["prog"]);
    let cfg = cli.load_and_merge().expect("load");
    assert_eq!(cfg.num, Some(5));
}

#[test]
fn exposes_prefix() {
    assert_eq!(Prefixed::prefix(), "CFG_");
}
