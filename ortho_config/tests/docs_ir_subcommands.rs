//! Tests for recursive subcommand metadata in `OrthoConfigDocs` IR generation.

use anyhow::{Result, anyhow, ensure};
use clap::{Args, Parser, Subcommand};
use ortho_config::docs::DocMetadata;
use ortho_config::docs::OrthoConfigDocs;
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct RootWithSubcommands {
    #[serde(skip)]
    #[command(subcommand)]
    command: RootCommands,
    #[arg(long)]
    global: String,
}

#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
enum RootCommands {
    Zebra(ZebraArgs),
    Run(RunArgs),
    #[command(name = "take-leave")]
    Leave(TakeLeaveArgs),
    Admin(AdminArgs),
}

impl Default for RootCommands {
    fn default() -> Self {
        Self::Run(RunArgs::default())
    }
}

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct ZebraArgs {
    #[arg(long)]
    stripes: u8,
}

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct RunArgs {
    #[arg(long)]
    name: String,
}

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct TakeLeaveArgs {
    #[arg(long)]
    parting: String,
}

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct AdminArgs {
    #[serde(skip)]
    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
enum AdminCommands {
    Audit(AuditArgs),
}

impl Default for AdminCommands {
    fn default() -> Self {
        Self::Audit(AuditArgs::default())
    }
}

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct AuditArgs {
    #[arg(long)]
    dry_run: bool,
}

#[fixture]
fn subcommand_metadata() -> DocMetadata {
    RootWithSubcommands::get_doc_metadata()
}

#[rstest]
fn test_subcommand_metadata_is_populated(subcommand_metadata: DocMetadata) -> Result<()> {
    let names = subcommand_metadata
        .subcommands
        .iter()
        .map(|entry| entry.app_name.as_str())
        .collect::<Vec<_>>();

    ensure!(
        names == ["zebra", "run", "take-leave", "admin"],
        "expected recursive subcommands in declaration order, got {names:?}",
    );
    Ok(())
}

#[rstest]
fn test_subcommand_selector_is_not_a_field(subcommand_metadata: DocMetadata) -> Result<()> {
    let field_names = subcommand_metadata
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    ensure!(
        field_names == ["global"],
        "expected only configuration fields in parent metadata, got {field_names:?}",
    );
    Ok(())
}

#[rstest]
fn test_nested_subcommand_metadata_is_populated(subcommand_metadata: DocMetadata) -> Result<()> {
    let admin = subcommand_metadata
        .subcommands
        .iter()
        .find(|entry| entry.app_name == "admin")
        .ok_or_else(|| anyhow!("missing admin metadata"))?;
    let nested_names = admin
        .subcommands
        .iter()
        .map(|entry| entry.app_name.as_str())
        .collect::<Vec<_>>();

    ensure!(
        nested_names == ["audit"],
        "expected nested admin subcommands, got {nested_names:?}",
    );
    Ok(())
}
