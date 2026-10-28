//! Tests for renderer metadata in the documentation IR.
//!
//! This exercises the authoring half of the contract: the IR schema is strict,
//! so unknown tokens must be rejected rather than tolerated. The tolerant
//! normalisation the wire format promises lives in `agent_context` and is
//! covered there; keeping the two apart is what stops a lenient change here
//! from silently weakening authoring.

use anyhow::{Result, anyhow, ensure};
use ortho_config::docs::{
    DocMetadata, ORTHO_DOCS_IR_VERSION, RenderWidth, RendererField, RendererFieldLocalization,
    RendererMetadata, RendererRecord, StreamContent, TogglePolicy, TtyPolicy,
};
use rstest::rstest;
use serde_json::{Value, json};

/// Builds a minimal `DocMetadata` JSON document without a renderer block.
///
/// Every required field is present and no optional metadata is declared, so
/// the assertions below isolate the `renderer` block under test.
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

/// Attaches a `renderer` block to the minimal document JSON object.
fn doc_json_with_renderer(renderer: Value) -> Result<Value> {
    let mut document = minimal_doc_json();
    document
        .as_object_mut()
        .ok_or_else(|| anyhow!("minimal document should be a JSON object"))?
        .insert("renderer".to_owned(), renderer);
    Ok(document)
}

/// Decodes a document and returns its renderer block.
fn decode_renderer(renderer: Value) -> Result<RendererMetadata> {
    let metadata: DocMetadata = serde_json::from_value(doc_json_with_renderer(renderer)?)?;
    metadata
        .renderer
        .ok_or_else(|| anyhow!("expected renderer metadata to be present"))
}

/// An absent block decodes to `None` rather than an invented default.
#[test]
fn test_renderer_is_absent_when_not_declared() -> Result<()> {
    let metadata: DocMetadata = serde_json::from_value(minimal_doc_json())?;
    ensure!(
        metadata.renderer.is_none(),
        "an undeclared renderer must stay absent, not default: {:?}",
        metadata.renderer
    );
    Ok(())
}

/// An empty object decodes to a block with no declared leaves.
///
/// This pins that the block itself is optional per record: declaring
/// `renderer: {}` must not manufacture support claims.
#[test]
fn test_empty_renderer_block_decodes_to_absent_records() -> Result<()> {
    let renderer = decode_renderer(json!({}))?;
    ensure!(
        renderer.human.is_none(),
        "human: {:?}",
        renderer.human.is_some()
    );
    ensure!(
        renderer.machine.is_none(),
        "machine: {:?}",
        renderer.machine.is_some()
    );
    Ok(())
}

/// A fully declared record decodes into its typed IR values.
///
/// The expected values are constructed as typed IR rather than compared as
/// JSON, so a changed wire token fails here instead of matching loosely.
#[test]
fn test_fully_declared_record_decodes() -> Result<()> {
    let renderer = decode_renderer(json!({
        "human": {
            "supported": true,
            "tty": "adapts",
            "closed_stdin": "eof",
            "colour": "auto",
            "emoji": "never",
            "progress": "always",
            "pager": "never",
            "width": { "kind": "fixed", "columns": 80 },
            "accessibility": "text_equivalent",
            "plain": "available",
            "stdout": "text",
            "stderr": "mixed",
            "progress_inheritance": "allowed",
            "subprocess_inheritance": "forbidden",
            "fields": [
                { "id": "error_code", "localization": "non_localized" }
            ]
        }
    }))?;

    let human = renderer
        .human
        .ok_or_else(|| anyhow!("expected a human record"))?;
    ensure!(human.supported == Some(true), "supported");
    ensure!(human.tty == Some(TtyPolicy::Adapts), "tty: {:?}", human.tty);
    ensure!(human.colour == Some(TogglePolicy::Auto), "colour");
    ensure!(human.emoji == Some(TogglePolicy::Never), "emoji");
    ensure!(human.progress == Some(TogglePolicy::Always), "progress");
    ensure!(human.pager == Some(TogglePolicy::Never), "pager");
    ensure!(
        matches!(human.width, Some(RenderWidth::Fixed { columns }) if columns.get() == 80),
        "width: {:?}",
        human.width
    );
    ensure!(human.stdout == Some(StreamContent::Text), "stdout");
    ensure!(human.stderr == Some(StreamContent::Mixed), "stderr");
    ensure!(
        human.fields
            == [RendererField {
                id: "error_code".to_owned(),
                localization: RendererFieldLocalization::NonLocalized,
            }],
        "fields: {:?}",
        human.fields
    );
    ensure!(renderer.machine.is_none(), "no machine record was declared");
    Ok(())
}

/// Every accepted width shape decodes to its matching IR variant.
#[rstest]
#[case::terminal(json!({ "kind": "terminal" }))]
#[case::unbounded(json!({ "kind": "unbounded" }))]
fn test_named_width_shapes_decode(#[case] width: Value) -> Result<()> {
    let renderer = decode_renderer(json!({ "human": { "width": width } }))?;
    let human = renderer
        .human
        .ok_or_else(|| anyhow!("expected a human record"))?;
    ensure!(human.width.is_some(), "expected a width to be decoded");
    Ok(())
}

/// A renderer block round-trips through serialization unchanged.
///
/// The comparison is on the decoded value, not the JSON text, so key order
/// stays free while every declared leaf is still checked.
#[test]
fn test_renderer_round_trips() -> Result<()> {
    let declared = RendererMetadata {
        human: Some(RendererRecord {
            supported: Some(true),
            colour: Some(TogglePolicy::Always),
            fields: vec![RendererField {
                id: "msg".to_owned(),
                localization: RendererFieldLocalization::Localized,
            }],
            ..RendererRecord::default()
        }),
        machine: Some(RendererRecord {
            supported: Some(false),
            stdout: Some(StreamContent::Structured),
            ..RendererRecord::default()
        }),
    };

    let document = doc_json_with_renderer(serde_json::to_value(&declared)?)?;
    let metadata: DocMetadata = serde_json::from_value(document)?;
    ensure!(
        metadata.renderer == Some(declared),
        "round trip changed the renderer block: {:?}",
        metadata.renderer
    );
    Ok(())
}

/// Unknown object keys inside a record are ignored, not rejected.
///
/// Forward compatibility is deliberate on both schemas: a newer producer may
/// add a key, and an older reader must keep working.
#[test]
fn test_unknown_record_key_is_ignored() -> Result<()> {
    let renderer = decode_renderer(json!({
        "human": { "colour": "auto", "hyperlink": "always" }
    }))?;
    let human = renderer
        .human
        .ok_or_else(|| anyhow!("expected a human record"))?;
    ensure!(
        human.colour == Some(TogglePolicy::Auto),
        "the known leaf must still decode"
    );
    Ok(())
}

/// An unknown policy token is rejected in the authoring IR.
///
/// This is the strict half of the split contract: the wire format normalizes
/// unknown tokens to `unknown`, but authoring must fail so a typo cannot ship.
#[rstest]
#[case::tty(json!({ "tty": "sometimes" }))]
#[case::colour(json!({ "colour": "on" }))]
#[case::stdout(json!({ "stdout": "binary" }))]
#[case::accessibility(json!({ "accessibility": "full" }))]
fn test_unknown_policy_token_is_rejected(#[case] record: Value) -> Result<()> {
    let result = decode_renderer(json!({ "human": record }));
    ensure!(
        result.is_err(),
        "an unknown token must be rejected in the authoring IR, but it decoded"
    );
    Ok(())
}

/// An unknown width spelling is rejected.
#[test]
fn test_unknown_width_shape_is_rejected() -> Result<()> {
    let result = decode_renderer(json!({ "human": { "width": { "kind": "wide" } } }));
    ensure!(result.is_err(), "an unknown width must be rejected");
    Ok(())
}

/// A zero fixed width is rejected by the `NonZeroU16` column count.
#[test]
fn test_zero_fixed_width_is_rejected() -> Result<()> {
    let result =
        decode_renderer(json!({ "human": { "width": { "kind": "fixed", "columns": 0 } } }));
    ensure!(
        result.is_err(),
        "a zero column count must be rejected, not normalized"
    );
    Ok(())
}

/// An unknown field-localization token is rejected.
#[test]
fn test_unknown_field_localization_is_rejected() -> Result<()> {
    let result = decode_renderer(json!({
        "human": { "fields": [{ "id": "code", "localization": "maybe" }] }
    }));
    ensure!(
        result.is_err(),
        "an unknown localization token must be rejected"
    );
    Ok(())
}
