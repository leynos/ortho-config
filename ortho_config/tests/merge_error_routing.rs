//! Integration tests verifying merge failures route through `OrthoError::Merge`.
//!
//! Ensures deserialisation errors during the merge phase (not gathering phase)
//! produce the correct error variant, establishing a clear semantic distinction.
//!
//! Note: `MergeErrorSample` is intentionally defined locally rather than imported
//! from `rstest_bdd/fixtures.rs` to keep these integration tests self-contained.
//! Both definitions must remain synchronised (port: u16 with default 8080).

use anyhow::{Result, ensure};
use ortho_config::{MergeComposer, OrthoConfig, OrthoError};
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;

/// Minimal struct used by merge error routing tests.
///
/// Mirrors `MergeErrorSample` in `rstest_bdd/fixtures.rs`.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "TEST")]
struct MergeErrorSample {
    #[ortho_config(default = 8080)]
    port: u16,
}

#[expect(
    dead_code,
    reason = "Fields used only for deserialization testing, not directly accessed"
)]
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "TEST")]
struct VecAppendSample {
    items: Vec<u32>,
}

/// Tests that type mismatches during final deserialization produce `Merge` errors.
#[rstest]
fn merge_deserialization_error_produces_merge_variant() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "port": "not_a_number"
    }));

    let result = MergeErrorSample::merge_from_layers(composer.layers());
    let error = result.expect_err("expected merge to fail due to invalid port type");

    ensure!(
        matches!(&*error, OrthoError::Merge { .. }),
        "expected Merge error variant, got {error:?}"
    );

    Ok(())
}

/// Tests that vector append deserialization errors produce `Merge` errors.
#[rstest]
fn vector_append_deserialization_error_produces_merge_variant() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "items": ["not_a_number"] }));

    let result = VecAppendSample::merge_from_layers(composer.layers());
    let error = result.expect_err("expected merge to fail due to invalid vector element");

    ensure!(
        matches!(&*error, OrthoError::Merge { .. }),
        "expected Merge error variant for vector deserialization, got {error:?}"
    );

    Ok(())
}

/// Tests that successful merges continue to work correctly.
#[rstest]
fn successful_merge_produces_correct_result() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "port": 8080
    }));
    composer.push_environment(json!({
        "port": 9090
    }));

    let config = MergeErrorSample::merge_from_layers(composer.layers())?;

    ensure!(
        config.port == 9090,
        "expected port 9090, got {}",
        config.port
    );

    Ok(())
}

/// Tests that error messages contain helpful context.
#[rstest]
fn merge_error_contains_helpful_message() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "port": "invalid"
    }));

    let result = MergeErrorSample::merge_from_layers(composer.layers());
    let error = result.expect_err("expected merge to fail");

    let message = error.to_string();
    ensure!(
        message.to_lowercase().contains("merge"),
        "error message should reference merge context: {message}"
    );

    Ok(())
}
