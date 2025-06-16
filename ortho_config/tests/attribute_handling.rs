//! Tests for attribute handling in the derive macro.

#![allow(non_snake_case)]

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
#[allow(dead_code)]
struct CustomCli {
    #[ortho_config(cli_long = "my-val")]
    value: String,
}

#[derive(Debug, Deserialize, OrthoConfig)]
#[allow(dead_code)]
#[ortho_config(prefix = "CFG_")]
struct Prefixed {
    value: String,
}

#[derive(Debug, Deserialize, OrthoConfig)]
#[allow(dead_code)]
struct Defaulted {
    #[ortho_config(default = 5)]
    num: u32,
}

#[test]
fn uses_custom_cli_long() {
    use clap::Parser;
    let cli = CustomCliCli::parse_from(["prog", "--my-val", "42"]);
    assert_eq!(cli.value.as_deref(), Some("42"));
}

#[test]
fn env_prefix_is_used() {
    figment::Jail::expect_with(|j| {
        j.set_env("CFG_VALUE", "env");
        let cfg = Prefixed::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.value, "env");
        Ok(())
    });
}

#[test]
fn default_value_applied() {
    let cfg = Defaulted::load_from_iter(["prog"]).expect("load");
    assert_eq!(cfg.num, 5);
}

#[test]
fn exposes_prefix() {
    assert_eq!(Prefixed::prefix(), "CFG_");
}
