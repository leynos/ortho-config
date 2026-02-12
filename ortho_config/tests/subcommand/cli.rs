//! CLI precedence and required-value behaviour tests.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use rstest::{fixture, rstest};
use serde::Deserialize;
use test_helpers::env;

use super::util::with_merged_subcommand_cli;

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq)]
#[command(name = "test")]
struct RequiredCli {
    #[arg(long, required = true)]
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq)]
#[command(name = "test")]
struct OptionalCli {
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<String>,
}

#[fixture]
fn cli_ref_id() -> RequiredCli {
    RequiredCli {
        ref_id: Some("cli".into()),
    }
}

#[fixture]
fn env_scope() -> env::EnvScope {
    env::scope_with(|lock| vec![lock.remove_var("APP_CMDS_TEST_REF_ID")])
}

#[rstest]
fn cli_only_values_are_accepted(cli_ref_id: RequiredCli) -> Result<()> {
    let merged: RequiredCli =
        with_merged_subcommand_cli(|_j| Ok(()), &cli_ref_id).map_err(|err| anyhow!(err))?;
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

#[rstest]
fn conflicting_values_cli_takes_precedence(
    cli_ref_id: RequiredCli,
    env_scope: env::EnvScope,
) -> Result<()> {
    let _scope = env_scope;
    let merged: RequiredCli = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nref_id = \"config\"")?;
            j.set_env("APP_CMDS_TEST_REF_ID", "env");
            Ok(())
        },
        &cli_ref_id,
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        merged.ref_id.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.ref_id
    );
    Ok(())
}

#[rstest]
fn env_value_used_when_cli_missing(env_scope: env::EnvScope) -> Result<()> {
    let _scope = env_scope;
    let cli = OptionalCli { ref_id: None };
    let merged: OptionalCli = with_merged_subcommand_cli(
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
