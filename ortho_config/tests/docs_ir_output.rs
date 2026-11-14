//! Tests for structured-output metadata in the documentation IR.
//!
//! The IR is the strict half of the contract: unknown stdout tokens and a
//! `null` class vector are rejected, because an authoring error must not reach
//! a consumer as a silently different declaration. The tolerant reading of
//! future tokens belongs to the agent-context wire enum and is covered there.
//!
//! Two properties matter more than the shape itself. Declaration provenance is
//! preserved — `supports_json: false` is not the same as an absent member — and
//! an absent member is omitted from the wire rather than written as `null`, so
//! the docs IR and its `cargo-orthohelp` mirror serialize identical JSON.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{
    DocMetadata, ExitClass, ORTHO_DOCS_IR_VERSION, OutputMetadata, StdoutPurpose,
};
use ortho_config::{OrthoConfig, OrthoConfigDocs};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Builds a minimal `DocMetadata` JSON document without an output block.
///
/// Every required field is present and no optional metadata is declared, so the
/// assertions below isolate the `output` block under test.
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

/// Attaches an `output` block to the minimal document JSON object.
fn doc_json_with_output(output: Value) -> Result<Value> {
    let mut document = minimal_doc_json();
    document
        .as_object_mut()
        .ok_or_else(|| anyhow!("minimal document should be a JSON object"))?
        .insert("output".to_owned(), output);
    Ok(document)
}

/// Decodes a document and returns its output block.
fn decode_output(output: Value) -> Result<OutputMetadata> {
    let metadata: DocMetadata = serde_json::from_value(doc_json_with_output(output)?)?;
    metadata
        .output
        .ok_or_else(|| anyhow!("expected output metadata to be present"))
}

/// An absent block decodes to `None` rather than an invented default.
#[test]
fn test_output_is_absent_when_not_declared() -> Result<()> {
    let metadata: DocMetadata = serde_json::from_value(minimal_doc_json())?;
    ensure!(
        metadata.output.is_none(),
        "an undeclared output block must stay absent, not default: {:?}",
        metadata.output
    );
    Ok(())
}

/// An explicit `null` reads as absence, so both spellings agree.
#[test]
fn test_null_output_block_decodes_to_none() -> Result<()> {
    let metadata: DocMetadata = serde_json::from_value(doc_json_with_output(Value::Null)?)?;
    ensure!(
        metadata.output.is_none(),
        "a null block must not become a declared record: {:?}",
        metadata.output
    );
    Ok(())
}

/// An empty object decodes to a block that claims nothing.
#[test]
fn test_empty_output_block_claims_nothing() -> Result<()> {
    let output = decode_output(json!({}))?;
    ensure!(
        output == OutputMetadata::default(),
        "an empty block should declare no leaf: {output:?}"
    );
    Ok(())
}

/// A fully declared block decodes into its typed IR values.
#[test]
fn test_fully_declared_output_decodes() -> Result<()> {
    let output = decode_output(json!({
        "supports_json": true,
        "stdout": "data",
        "stderr_diagnostics": false,
        "exit_classes": [
            {"name": "success", "meaning": "The command completed"},
            {"name": "usage", "meaning": "The invocation was invalid"}
        ]
    }))?;

    let expected = OutputMetadata {
        supports_json: Some(true),
        stdout: Some(StdoutPurpose::Data),
        stderr_diagnostics: Some(false),
        exit_classes: vec![
            ExitClass {
                name: "success".to_owned(),
                meaning: "The command completed".to_owned(),
            },
            ExitClass {
                name: "usage".to_owned(),
                meaning: "The invocation was invalid".to_owned(),
            },
        ],
    };
    ensure!(output == expected, "decoded block: {output:?}");
    Ok(())
}

/// Every accepted stdout token has its own decode case, so a token that maps
/// to the wrong variant cannot hide behind a shared spelling.
#[rstest]
#[case::data("data", StdoutPurpose::Data)]
#[case::path("path", StdoutPurpose::Path)]
#[case::summary("summary", StdoutPurpose::Summary)]
#[case::none("none", StdoutPurpose::None)]
fn test_stdout_tokens_decode(#[case] token: &str, #[case] expected: StdoutPurpose) -> Result<()> {
    let output = decode_output(json!({"stdout": token}))?;

    ensure!(
        output.stdout == Some(expected),
        "expected {expected:?} for {token:?}, got {:?}",
        output.stdout
    );
    Ok(())
}

/// A `null` scalar reads as undeclared, matching omission.
#[rstest]
#[case::supports_json(json!({"supports_json": null}))]
#[case::stdout(json!({"stdout": null}))]
#[case::stderr_diagnostics(json!({"stderr_diagnostics": null}))]
fn test_null_scalars_read_as_undeclared(#[case] block: Value) -> Result<()> {
    let output = decode_output(block)?;

    ensure!(
        output == OutputMetadata::default(),
        "a null scalar should stay undeclared: {output:?}"
    );
    Ok(())
}

/// A `null` class vector is rejected: it is a malformed list, not an absence.
#[test]
fn test_null_class_vector_is_rejected() -> Result<()> {
    let decoded: Result<DocMetadata, _> =
        serde_json::from_value(doc_json_with_output(json!({"exit_classes": null}))?);
    let error = decoded
        .err()
        .ok_or_else(|| anyhow!("a null class vector must be rejected"))?;
    ensure!(
        error.to_string().contains("expected a sequence"),
        "a null list must fail as a type error, not decode as empty: {error}"
    );
    Ok(())
}

/// An unknown stdout token is rejected because the IR is the strict schema.
#[test]
fn test_unknown_stdout_token_is_rejected() -> Result<()> {
    let decoded: Result<DocMetadata, _> = serde_json::from_value(doc_json_with_output(
        json!({"stdout": "structured_stream"}),
    )?);
    let error = decoded
        .err()
        .ok_or_else(|| anyhow!("an unknown stdout token must be rejected"))?;
    ensure!(
        error.to_string().contains("structured_stream"),
        "the diagnostic should name the rejected token: {error}"
    );
    Ok(())
}

/// Undeclared leaves are omitted on the wire, so both schema owners produce
/// the same JSON and an absent member stays distinguishable from `false`.
#[test]
fn test_undeclared_leaves_are_omitted_when_serialized() -> Result<()> {
    let metadata: DocMetadata = serde_json::from_value(doc_json_with_output(json!({}))?)?;
    let encoded = serde_json::to_value(&metadata)?;
    let block = encoded
        .get("output")
        .ok_or_else(|| anyhow!("the declared empty block should serialize"))?;

    ensure!(
        block == &json!({}),
        "an empty block should serialize as an empty object, got: {block}"
    );
    Ok(())
}

/// A declared refusal survives the round trip as `Some(false)`.
#[test]
fn test_declared_false_survives_serialization() -> Result<()> {
    let metadata: DocMetadata = serde_json::from_value(doc_json_with_output(
        json!({"supports_json": false, "stderr_diagnostics": false}),
    )?)?;
    let encoded = serde_json::to_value(&metadata)?;
    let block = encoded
        .get("output")
        .ok_or_else(|| anyhow!("the declared block should serialize"))?;

    ensure!(
        block == &json!({"supports_json": false, "stderr_diagnostics": false}),
        "a declared refusal must stay on the wire, got: {block}"
    );
    Ok(())
}

/// Class order and text are preserved verbatim, including non-ASCII meanings.
#[test]
fn test_exit_classes_round_trip_verbatim() -> Result<()> {
    let declared = json!({
        "exit_classes": [
            {"name": "usage", "meaning": "The invocation was invalid"},
            {"name": "success", "meaning": "The command completed — nothing to do"}
        ]
    });
    let metadata: DocMetadata = serde_json::from_value(doc_json_with_output(declared.clone())?)?;
    let encoded = serde_json::to_value(&metadata)?;

    ensure!(
        encoded.get("output") == Some(&declared),
        "classes should round trip in author order and text, got: {:?}",
        encoded.get("output")
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Output-block emission from the derive attribute surface.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    output(
        supports_json = true,
        stdout = "data",
        stderr_diagnostics = true,
        exit_class(name = "success", meaning = "The command completed"),
        exit_class(name = "usage", meaning = "The invocation was invalid")
    )
)]
struct DeclaredOutputConfig {
    value: u8,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP", output(supports_json = false, stdout = "none"))]
struct RefusedJsonConfig {
    value: u8,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP", output())]
struct EmptyOutputConfig {
    value: u8,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP")]
struct UndeclaredOutputConfig {
    value: u8,
}

/// A fully declared attribute reaches the generated IR unchanged.
#[test]
fn test_derive_emits_declared_output() -> Result<()> {
    let metadata = DeclaredOutputConfig::get_doc_metadata();
    let output = metadata
        .output
        .ok_or_else(|| anyhow!("expected an output block from the derive"))?;

    ensure!(output.supports_json == Some(true), "JSON support");
    ensure!(
        output.stdout == Some(StdoutPurpose::Data),
        "stdout purpose: {:?}",
        output.stdout
    );
    ensure!(
        output.stderr_diagnostics == Some(true),
        "diagnostic stderr: {:?}",
        output.stderr_diagnostics
    );
    let names = output
        .exit_classes
        .iter()
        .map(|class| class.name.as_str())
        .collect::<Vec<_>>();
    ensure!(
        names == ["success", "usage"],
        "declaration order should reach the IR, got: {names:?}"
    );
    Ok(())
}

/// `output()` declares a block that makes no claim; it does not vanish.
#[test]
fn test_derive_emits_empty_block_for_empty_group() -> Result<()> {
    let metadata = EmptyOutputConfig::get_doc_metadata();
    let output = metadata
        .output
        .ok_or_else(|| anyhow!("an explicit empty group should still emit a block"))?;

    ensure!(
        output == OutputMetadata::default(),
        "an empty group should declare nothing: {output:?}"
    );
    Ok(())
}

/// A derive without an `output` group emits no block at all.
#[test]
fn test_derive_keeps_output_none_when_undeclared() -> Result<()> {
    let metadata = UndeclaredOutputConfig::get_doc_metadata();
    ensure!(
        metadata.output.is_none(),
        "expected no output block for an undeclared config: {:?}",
        metadata.output
    );
    Ok(())
}

/// A declared refusal is emitted as `Some(false)`, never dropped.
#[test]
fn test_derive_preserves_declared_refusal() -> Result<()> {
    let metadata = RefusedJsonConfig::get_doc_metadata();
    let output = metadata
        .output
        .ok_or_else(|| anyhow!("expected an output block from the derive"))?;

    ensure!(
        output.supports_json == Some(false),
        "a refusal must survive as `Some(false)`, got: {:?}",
        output.supports_json
    );
    ensure!(
        output.stdout == Some(StdoutPurpose::None),
        "stdout purpose: {:?}",
        output.stdout
    );
    Ok(())
}
