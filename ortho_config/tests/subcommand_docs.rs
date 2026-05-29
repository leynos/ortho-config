//! Tests for recursive subcommand documentation metadata.

use anyhow::{Result, ensure};
use ortho_config::docs::{
    DocMetadata, HeadingIds, ORTHO_DOCS_IR_VERSION, OrthoConfigDocs, SectionsMetadata,
};
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(clap::Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct RunArgs {
    #[arg(long)]
    name: String,
}

#[derive(clap::Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct TakeLeaveArgs {
    #[arg(long)]
    parting: String,
}

#[derive(clap::Subcommand, OrthoConfigSubcommandDocs)]
enum Commands {
    Zebra(TakeLeaveArgs),
    Run(RunArgs),
    #[command(name = "take-leave")]
    Leave(TakeLeaveArgs),
}

struct NestedArgs;

impl OrthoConfigDocs for NestedArgs {
    fn get_doc_metadata() -> DocMetadata {
        let mut metadata = empty_metadata("nested-args");
        metadata.subcommands = vec![empty_metadata("inner-child")];
        metadata
    }
}

#[derive(OrthoConfigSubcommandDocs)]
enum NestedCommands {
    Outer(NestedArgs),
}

fn headings() -> HeadingIds {
    HeadingIds {
        name: "ortho.headings.name".to_owned(),
        synopsis: "ortho.headings.synopsis".to_owned(),
        description: "ortho.headings.description".to_owned(),
        options: "ortho.headings.options".to_owned(),
        environment: "ortho.headings.environment".to_owned(),
        files: "ortho.headings.files".to_owned(),
        precedence: "ortho.headings.precedence".to_owned(),
        exit_status: "ortho.headings.exit-status".to_owned(),
        examples: "ortho.headings.examples".to_owned(),
        see_also: "ortho.headings.see-also".to_owned(),
        commands: Some("ortho.headings.commands".to_owned()),
    }
}

fn empty_metadata(app_name: &str) -> DocMetadata {
    DocMetadata {
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: app_name.to_owned(),
        bin_name: None,
        about_id: format!("{app_name}.about"),
        synopsis_id: None,
        sections: SectionsMetadata {
            headings_ids: headings(),
            discovery: None,
            precedence: None,
            examples: Vec::new(),
            links: Vec::new(),
            notes: Vec::new(),
        },
        fields: Vec::new(),
        subcommands: Vec::new(),
        windows: None,
    }
}

fn field_values<'a>(
    metadata: &'a [DocMetadata],
    extract: impl Fn(&'a DocMetadata) -> &'a str,
) -> Vec<&'a str> {
    metadata.iter().map(extract).collect()
}

#[rstest]
fn subcommand_docs_preserve_declaration_order() -> Result<()> {
    let metadata = Commands::get_subcommand_doc_metadata();
    let names = field_values(&metadata, |e| e.app_name.as_str());
    ensure!(
        names == ["zebra", "run", "take-leave"],
        "expected declaration order and clap labels, got {names:?}",
    );
    Ok(())
}

#[rstest]
fn subcommand_docs_regenerate_about_ids() -> Result<()> {
    let metadata = Commands::get_subcommand_doc_metadata();
    let about_ids = field_values(&metadata, |e| e.about_id.as_str());
    ensure!(
        about_ids == ["zebra.about", "run.about", "take-leave.about"],
        "expected about IDs to follow command labels, got {about_ids:?}",
    );
    Ok(())
}

#[rstest]
fn subcommand_docs_preserve_child_metadata() -> Result<()> {
    let metadata = Commands::get_subcommand_doc_metadata();
    let run = metadata
        .iter()
        .find(|entry| entry.app_name == "run")
        .ok_or_else(|| anyhow::anyhow!("missing run metadata"))?;

    ensure!(run.fields.len() == 1, "expected RunArgs field metadata");
    let field = run
        .fields
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing RunArgs field metadata"))?;
    ensure!(field.name == "name", "unexpected field metadata");
    Ok(())
}

#[rstest]
fn subcommand_docs_preserve_nested_subcommands() -> Result<()> {
    ensure!(
        matches!(NestedCommands::Outer(NestedArgs), NestedCommands::Outer(_)),
        "expected nested command variant to be constructible",
    );
    let metadata = NestedCommands::get_subcommand_doc_metadata();
    let outer = metadata
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing outer command metadata"))?;
    ensure!(metadata.len() == 1, "expected one outer subcommand");
    ensure!(
        outer.app_name == "outer",
        "expected outer command label override"
    );
    ensure!(
        outer.subcommands.len() == 1,
        "expected nested child metadata to be preserved",
    );
    let inner = outer
        .subcommands
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing inner child metadata"))?;
    ensure!(
        inner.app_name == "inner-child",
        "unexpected nested child metadata",
    );
    Ok(())
}
