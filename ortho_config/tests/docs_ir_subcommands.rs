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

// ---------------------------------------------------------------------------
// Behaviour metadata flows through ADR-005 subcommand delegation (Milestone C).
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP_",
    behaviour(interaction = "interactive", mutation = "delete", bypass = "--force")
)]
struct PurgeArgs {
    #[arg(long)]
    recursive: bool,
}

#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct PlainArgs {
    #[arg(long)]
    name: String,
}

#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
enum DelegatedCommands {
    Purge(PurgeArgs),
    Plain(PlainArgs),
}

impl Default for DelegatedCommands {
    fn default() -> Self {
        Self::Plain(PlainArgs::default())
    }
}

#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct DelegatedRoot {
    #[serde(skip)]
    #[command(subcommand)]
    command: DelegatedCommands,
}

fn subcommand_by_name<'a>(metadata: &'a DocMetadata, name: &str) -> Result<&'a DocMetadata> {
    metadata
        .subcommands
        .iter()
        .find(|entry| entry.app_name == name)
        .ok_or_else(|| anyhow!("missing subcommand {name}"))
}

#[rstest]
fn test_subcommand_behaviour_flows_through_delegation() -> Result<()> {
    let metadata = DelegatedRoot::get_doc_metadata();
    let purge = subcommand_by_name(&metadata, "purge")?;
    let behaviour = purge
        .behaviour
        .as_ref()
        .ok_or_else(|| anyhow!("expected purge behaviour via delegation"))?;

    ensure!(
        behaviour.interaction == Some(ortho_config::docs::InteractionKind::Interactive),
        "expected interactive via delegation, got {:?}",
        behaviour.interaction
    );
    ensure!(
        behaviour.mutation == Some(ortho_config::docs::MutationKind::Delete),
        "expected delete via delegation, got {:?}",
        behaviour.mutation
    );
    ensure!(
        behaviour.bypass.as_deref() == Some("--force"),
        "expected --force via delegation, got {:?}",
        behaviour.bypass
    );
    Ok(())
}

#[rstest]
fn test_subcommand_behaviour_is_none_when_undeclared() -> Result<()> {
    let metadata = DelegatedRoot::get_doc_metadata();
    let plain = subcommand_by_name(&metadata, "plain")?;
    ensure!(
        plain.behaviour.is_none(),
        "expected undeclared subcommand behaviour to stay None"
    );
    Ok(())
}

// A single args struct reused as both root and subcommand carries identical
// behaviour in both positions. This is correct by design (struct-level
// metadata is intrinsic to the args struct, not its position in the tree);
// the test pins the behaviour so nobody "fixes" it later.
#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP_",
    behaviour(interaction = "non_interactive", mutation = "write")
)]
struct ReusedArgs {
    #[arg(long)]
    target: String,
}

#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
enum ReusedCommands {
    Do(ReusedArgs),
}

impl Default for ReusedCommands {
    fn default() -> Self {
        Self::Do(ReusedArgs::default())
    }
}

#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct ReusedRoot {
    #[serde(skip)]
    #[command(subcommand)]
    command: ReusedCommands,
}

#[rstest]
fn test_reused_args_carries_behaviour_in_root_and_subcommand_positions() -> Result<()> {
    let as_root = ReusedArgs::get_doc_metadata();
    let root_metadata = ReusedRoot::get_doc_metadata();
    let as_subcommand = subcommand_by_name(&root_metadata, "do")?;

    ensure!(
        as_root.behaviour == as_subcommand.behaviour,
        "expected identical behaviour in both positions: root {:?}, subcommand {:?}",
        as_root.behaviour,
        as_subcommand.behaviour
    );
    let behaviour = as_root
        .behaviour
        .as_ref()
        .ok_or_else(|| anyhow!("expected declared behaviour"))?;
    ensure!(
        behaviour.interaction == Some(ortho_config::docs::InteractionKind::NonInteractive),
        "expected non_interactive in root position"
    );
    ensure!(
        behaviour.mutation == Some(ortho_config::docs::MutationKind::Write),
        "expected write in root position"
    );
    Ok(())
}
