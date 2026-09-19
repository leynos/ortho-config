//! Tests for behaviour metadata in the documentation IR.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{DocMetadata, InteractionKind, MutationKind, ORTHO_DOCS_IR_VERSION};
use ortho_config::{OrthoConfig, OrthoConfigDocs};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Builds a minimal `DocMetadata` JSON document for behaviour-block tests.
///
/// The document carries every required field but no optional metadata, so the
/// assertions below isolate the `behaviour` block under test.
fn minimal_doc_json() -> Value {
    json!({
        "ir_version": ORTHO_DOCS_IR_VERSION,
        "app_name": "demo-app",
        "bin_name": null,
        "about_id": "demo-app.about",
        "synopsis_id": null,
        "sections": {
            "headings_ids": {
                "name": "ortho.headings.name",
                "synopsis": "ortho.headings.synopsis",
                "description": "ortho.headings.description",
                "options": "ortho.headings.options",
                "environment": "ortho.headings.environment",
                "files": "ortho.headings.files",
                "precedence": "ortho.headings.precedence",
                "exit_status": "ortho.headings.exit_status",
                "examples": "ortho.headings.examples",
                "see_also": "ortho.headings.see_also"
            },
            "discovery": null,
            "precedence": null,
            "examples": [],
            "links": [],
            "notes": []
        },
        "fields": [],
        "subcommands": [],
        "windows": null
    })
}

/// Attaches a `behaviour` block to the minimal document JSON object.
fn doc_json_with_behaviour(behaviour: Value) -> Result<Value> {
    let mut document = minimal_doc_json();
    document
        .as_object_mut()
        .ok_or_else(|| anyhow!("minimal document should be a JSON object"))?
        .insert("behaviour".to_owned(), behaviour);
    Ok(document)
}

#[rstest]
fn test_behaviour_block_deserializes_when_fully_declared() -> Result<()> {
    let document = doc_json_with_behaviour(json!({
        "interaction": "interactive",
        "mutation": "delete",
        "bypass": "--force",
        "dry_run": "--dry-run"
    }))?;

    let metadata: DocMetadata = serde_json::from_value(document)?;
    let behaviour = metadata
        .behaviour
        .as_ref()
        .ok_or_else(|| anyhow!("expected behaviour metadata to be present"))?;

    ensure!(
        behaviour.interaction == Some(InteractionKind::Interactive),
        "expected interactive declaration, got {:?}",
        behaviour.interaction
    );
    ensure!(
        behaviour.mutation == Some(MutationKind::Delete),
        "expected delete declaration, got {:?}",
        behaviour.mutation
    );
    ensure!(
        behaviour.bypass.as_deref() == Some("--force"),
        "expected bypass --force, got {:?}",
        behaviour.bypass
    );
    ensure!(
        behaviour.dry_run.as_deref() == Some("--dry-run"),
        "expected dry_run --dry-run, got {:?}",
        behaviour.dry_run
    );
    Ok(())
}

#[rstest]
fn test_behaviour_block_is_none_when_absent() -> Result<()> {
    let metadata: DocMetadata = serde_json::from_value(minimal_doc_json())?;

    ensure!(
        metadata.behaviour.is_none(),
        "expected absent behaviour block to deserialize as None"
    );
    Ok(())
}

#[rstest]
fn test_behaviour_block_treats_partial_declarations_as_undeclared() -> Result<()> {
    let document = doc_json_with_behaviour(json!({
        "mutation": "read_only"
    }))?;

    let metadata: DocMetadata = serde_json::from_value(document)?;
    let behaviour = metadata
        .behaviour
        .as_ref()
        .ok_or_else(|| anyhow!("expected behaviour metadata to be present"))?;

    ensure!(
        behaviour.interaction.is_none(),
        "expected undeclared interaction, got {:?}",
        behaviour.interaction
    );
    ensure!(
        behaviour.mutation == Some(MutationKind::ReadOnly),
        "expected read_only declaration, got {:?}",
        behaviour.mutation
    );
    ensure!(behaviour.bypass.is_none(), "expected undeclared bypass");
    ensure!(behaviour.dry_run.is_none(), "expected undeclared dry_run");
    Ok(())
}

#[rstest]
fn test_behaviour_metadata_serializes_snake_case_wire_values() -> Result<()> {
    let document = doc_json_with_behaviour(json!({
        "interaction": "non_interactive",
        "mutation": "submit"
    }))?;

    let decoded: DocMetadata = serde_json::from_value(document)?;
    let behaviour = decoded
        .behaviour
        .as_ref()
        .ok_or_else(|| anyhow!("expected behaviour metadata to be present"))?;
    ensure!(
        behaviour.interaction == Some(InteractionKind::NonInteractive),
        "expected non_interactive wire value to decode"
    );
    ensure!(
        behaviour.mutation == Some(MutationKind::Submit),
        "expected submit wire value to decode"
    );

    let round_trip = serde_json::to_value(&decoded)?;
    let round_behaviour = round_trip
        .get("behaviour")
        .ok_or_else(|| anyhow!("expected behaviour in round-trip JSON"))?;
    ensure!(
        *round_behaviour
            == json!({
                "interaction": "non_interactive",
                "mutation": "submit",
                "bypass": null,
                "dry_run": null
            }),
        "expected behaviour block to round-trip with explicit nulls for undeclared keys"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Behaviour-block emission from the derive attribute surface (Milestone C).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    behaviour(
        interaction = "interactive",
        mutation = "delete",
        bypass = "--force",
        dry_run = "--dry-run"
    )
)]
struct DeclaredBehaviourConfig {
    value: u8,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP")]
struct UndeclaredBehaviourConfig {
    value: u8,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    behaviour(interaction = "non_interactive", mutation = "read_only")
)]
struct ReadOnlyNonInteractiveConfig {
    value: u8,
}

#[rstest]
#[case::fully_declared(
    DeclaredBehaviourConfig::get_doc_metadata,
    Some(InteractionKind::Interactive),
    Some(MutationKind::Delete),
    (Some("--force"), Some("--dry-run"))
)]
#[case::partial(
    ReadOnlyNonInteractiveConfig::get_doc_metadata,
    Some(InteractionKind::NonInteractive),
    Some(MutationKind::ReadOnly),
    (None, None)
)]
fn test_derive_emits_behaviour_block(
    #[case] metadata_producer: fn() -> DocMetadata,
    #[case] expected_interaction: Option<InteractionKind>,
    #[case] expected_mutation: Option<MutationKind>,
    #[case] expected_flags: (Option<&str>, Option<&str>),
) -> Result<()> {
    let metadata = metadata_producer();
    let (expected_bypass, expected_dry_run) = expected_flags;
    let behaviour = metadata
        .behaviour
        .as_ref()
        .ok_or_else(|| anyhow!("expected behaviour metadata to be present"))?;

    ensure!(
        behaviour.interaction == expected_interaction,
        "expected interaction {:?}, got {:?}",
        expected_interaction,
        behaviour.interaction
    );
    ensure!(
        behaviour.mutation == expected_mutation,
        "expected mutation {:?}, got {:?}",
        expected_mutation,
        behaviour.mutation
    );
    ensure!(
        behaviour.bypass.as_deref() == expected_bypass,
        "expected bypass {:?}, got {:?}",
        expected_bypass,
        behaviour.bypass
    );
    ensure!(
        behaviour.dry_run.as_deref() == expected_dry_run,
        "expected dry_run {:?}, got {:?}",
        expected_dry_run,
        behaviour.dry_run
    );
    Ok(())
}

#[rstest]
fn test_derive_keeps_behaviour_none_when_undeclared() -> Result<()> {
    let metadata = UndeclaredBehaviourConfig::get_doc_metadata();
    ensure!(
        metadata.behaviour.is_none(),
        "expected no behaviour block for undeclared config"
    );
    Ok(())
}
