//! Prefix-handling tests for subcommand wrappers.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use super::util::with_merged_subcommand_cli_for;

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq)]
#[ortho_config(prefix = "APP_")]
#[command(name = "test")]
struct PrefixedCfg {
    foo: Option<String>,
}

#[test]
fn wrapper_uses_struct_prefix() -> Result<()> {
    let cfg: PrefixedCfg = with_merged_subcommand_cli_for(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"val\"")?;
            j.set_env("APP_CMDS_TEST_FOO", "env");
            Ok(())
        },
        &PrefixedCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.foo.as_deref() == Some("env"),
        "expected env, got {:?}",
        cfg.foo
    );
    Ok(())
}
