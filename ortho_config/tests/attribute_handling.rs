//! Tests for attribute handling in the derive macro.
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, ResultIntoFigment};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

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
fn uses_custom_cli_long() -> Result<()> {
    let cfg = CustomCli::load_from_iter(["prog", "--my-val", "42"])
        .map_err(|err| anyhow!("load custom cli: {err}"))?;
    ensure!(
        cfg.value.as_deref() == Some("42"),
        "expected value 42, got {:?}",
        cfg.value
    );
    Ok(())
}

#[rstest]
fn env_prefix_is_used() -> Result<()> {
    with_jail(|j| {
        j.set_env("CFG_VALUE", "env");
        let cfg = Prefixed::load_from_iter(["prog", "--value", "cli"]).to_figment()?;
        ensure!(
            cfg.value.as_deref() == Some("cli"),
            "expected CLI value cli, got {:?}",
            cfg.value
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn default_value_applied() -> Result<()> {
    let cfg =
        Defaulted::load_from_iter(["prog"]).map_err(|err| anyhow!("load defaulted: {err}"))?;
    ensure!(cfg.num == Some(5), "expected num 5, got {:?}", cfg.num);
    Ok(())
}

#[test]
fn exposes_prefix() {
    assert_eq!(Prefixed::prefix(), "CFG_");
}
