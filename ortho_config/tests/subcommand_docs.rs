//! Tests for recursive subcommand documentation metadata.
//!
//! The derived selector delegates each variant to its child's public
//! `OrthoConfigDocs` method, so the fixtures for the disclosure boundary live
//! in `support/subcommand_docs_projection.rs`; the last three cases here show
//! that a handwritten child cannot smuggle unprojected metadata through it.

use anyhow::{Result, ensure};
use ortho_config::docs::{DocMetadata, OrthoConfigDocs, ValueType};
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[path = "support/subcommand_docs_projection.rs"]
mod projection;

use projection::{MarkedArgs, MarkedCommands, PlainArgs, empty_metadata};

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
        about_ids == ["zebra-about", "run-about", "take-leave-about"],
        "expected about IDs to follow command labels, got {about_ids:?}",
    );
    Ok(())
}

/// A handwritten `OrthoConfigDocs` impl still answers path-aware queries.
///
/// The path-aware method has a default implementation, so a downstream type
/// that implements only the older entry point must keep compiling and
/// resolving exactly as before.
#[rstest]
fn handwritten_docs_implementations_retain_path_aware_compatibility() -> Result<()> {
    let parent_path = [String::from("root")];
    let metadata = NestedCommands::get_subcommand_doc_metadata_for_path(&parent_path);
    let outer = metadata
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing outer command metadata"))?;

    ensure!(
        outer.about_id == "nested-args.about",
        "handwritten metadata should use its provided fallback, got {}",
        outer.about_id,
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

/// Fails when a marked field anywhere in `document` still carries a value.
fn ensure_no_protected_value(document: &DocMetadata, boundary: &str) -> Result<()> {
    for field in &document.fields {
        if field.profile_field.is_none() {
            continue;
        }
        ensure!(
            field.default.is_none(),
            "{boundary}: the protected default survived for {}",
            field.name,
        );
        ensure!(
            field.examples.is_empty(),
            "{boundary}: protected field examples survived for {}",
            field.name,
        );
        let possible_values_survived = field
            .cli
            .as_ref()
            .is_some_and(|cli| !cli.possible_values.is_empty());
        ensure!(
            !possible_values_survived,
            "{boundary}: protected possible values survived for {}",
            field.name,
        );
        let nested_members_survived =
            matches!(field.value, Some(ValueType::Enum { ref variants }) if !variants.is_empty());
        ensure!(
            !nested_members_survived,
            "{boundary}: nested enum members survived for {}",
            field.name,
        );
    }
    for child in &document.subcommands {
        ensure_no_protected_value(child, boundary)?;
    }
    Ok(())
}

/// Fails unless the handwritten marked child really is unprojected.
///
/// Without this precondition, a projection that did nothing would pass the
/// assertions below.
fn ensure_marked_child_is_unprojected() -> Result<()> {
    // Constructing the variants keeps the selector honest: a rewrite that
    // stopped naming these children would fail here rather than pass silently.
    ensure!(
        matches!(
            MarkedCommands::Marked(MarkedArgs),
            MarkedCommands::Marked(MarkedArgs)
        ) && matches!(
            MarkedCommands::Plain(PlainArgs),
            MarkedCommands::Plain(PlainArgs)
        ),
        "the mixed selector must stay constructible",
    );

    let raw = MarkedArgs::get_doc_metadata();
    let default_retained = raw
        .fields
        .first()
        .is_some_and(|field| field.default.is_some());
    ensure!(
        default_retained,
        "the fixture must start with a retained protected default",
    );
    let example_retained = !raw.sections.examples.is_empty();
    ensure!(
        example_retained,
        "the fixture must start with a retained command example",
    );
    Ok(())
}

#[rstest]
fn handwritten_child_is_projected_by_the_entry_boundary() -> Result<()> {
    ensure_marked_child_is_unprojected()?;

    let metadata = MarkedCommands::get_subcommand_doc_metadata();
    let marked = metadata
        .iter()
        .find(|entry| entry.app_name == "marked")
        .ok_or_else(|| anyhow::anyhow!("missing marked subcommand metadata"))?;

    ensure_no_protected_value(marked, "entry boundary")?;
    ensure!(
        marked.sections.examples.is_empty(),
        "the marked command's own example must be suppressed",
    );
    ensure!(
        marked
            .fields
            .first()
            .is_some_and(|field| field.name == "token"),
        "the public field name must survive the boundary",
    );
    Ok(())
}

#[rstest]
fn handwritten_child_is_projected_by_the_path_aware_boundary() -> Result<()> {
    ensure_marked_child_is_unprojected()?;

    let parent_path = [String::from("root")];
    let metadata = MarkedCommands::get_subcommand_doc_metadata_for_path(&parent_path);
    let marked = metadata
        .iter()
        .find(|entry| entry.app_name == "marked")
        .ok_or_else(|| anyhow::anyhow!("missing marked subcommand metadata"))?;

    ensure_no_protected_value(marked, "path-aware boundary")?;
    ensure!(
        marked.sections.examples.is_empty(),
        "the marked command's own example must be suppressed",
    );
    Ok(())
}

#[rstest]
fn protected_sibling_suppresses_the_plain_command_examples() -> Result<()> {
    let raw_plain = PlainArgs::get_doc_metadata();
    ensure!(
        !raw_plain.sections.examples.is_empty(),
        "the plain sibling must start with a command example",
    );

    let metadata = MarkedCommands::get_subcommand_doc_metadata();
    let plain = metadata
        .iter()
        .find(|entry| entry.app_name == "plain")
        .ok_or_else(|| anyhow::anyhow!("missing plain subcommand metadata"))?;

    ensure!(
        plain.sections.examples.is_empty(),
        "a protected sibling must suppress the plain command's examples",
    );
    Ok(())
}
