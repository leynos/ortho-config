//! Tests for attribute handling in the derive macro.
#![allow(
    unfulfilled_lint_expectations,
    reason = "clippy::expect_used is denied globally; tests may not hit those branches"
)]
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct CustomCli {
    #[ortho_config(cli_long = "my-val")]
    value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "CFG_")]
struct Prefixed {
    value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct Defaulted {
    #[ortho_config(default = 5)]
    #[serde(skip_serializing_if = "Option::is_none")]
    num: Option<u32>,
}

#[rstest]
fn uses_custom_cli_long() {
    let cfg = CustomCli::load_from_iter(["prog", "--my-val", "42"]).expect("load");
    assert_eq!(cfg.value.as_deref(), Some("42"));
}

#[rstest]
fn env_prefix_is_used() {
    figment::Jail::expect_with(|j| {
        j.set_env("CFG_VALUE", "env");
        let cfg = Prefixed::load_from_iter(["prog", "--value", "cli"]).expect("load");
        assert_eq!(cfg.value.as_deref(), Some("cli"));
        Ok(())
    });
}

#[rstest]
fn default_value_applied() {
    let cfg = Defaulted::load_from_iter(["prog"]).expect("load");
    assert_eq!(cfg.num, Some(5));
}

#[test]
fn exposes_prefix() {
    assert_eq!(Prefixed::prefix(), "CFG_");
}
