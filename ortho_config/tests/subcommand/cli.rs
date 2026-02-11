//! CLI precedence and required-value behaviour tests.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use serde::Deserialize;

use super::util::with_merged_subcommand_cli;

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq)]
#[command(name = "test")]
struct RequiredCli {
    #[arg(long, required = true)]
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<String>,
}

#[rstest::rstest]
fn cli_only_values_are_accepted() -> Result<()> {
    let cli = RequiredCli {
        ref_id: Some("cli".into()),
    };
    let merged: RequiredCli =
        with_merged_subcommand_cli(|_j| Ok(()), &cli).map_err(|err| anyhow!(err))?;
    ensure!(
        merged.ref_id.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.ref_id
    );
    Ok(())
}

#[test]
fn error_when_required_cli_value_missing() {
    let result = RequiredCli::try_parse_from(["test"]);
    assert!(
        result.is_err(),
        "parsing should fail without required value"
    );
}

#[test]
fn conflicting_values_cli_takes_precedence() -> Result<()> {
    let cli = RequiredCli {
        ref_id: Some("cli".into()),
    };
    let merged: RequiredCli = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nref_id = \"config\"")?;
            j.set_env("APP_CMDS_TEST_REF_ID", "env");
            Ok(())
        },
        &cli,
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        merged.ref_id.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.ref_id
    );
    Ok(())
}

#[test]
fn env_value_used_when_cli_missing() -> Result<()> {
    let cli = RequiredCli { ref_id: None };
    let merged: RequiredCli = with_merged_subcommand_cli(
        |j| {
            j.set_env("APP_CMDS_TEST_REF_ID", "from-env");
            Ok(())
        },
        &cli,
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        merged.ref_id.as_deref() == Some("from-env"),
        "expected from-env, got {:?}",
        merged.ref_id
    );
    Ok(())
}
