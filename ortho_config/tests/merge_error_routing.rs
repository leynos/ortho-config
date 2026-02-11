//! Integration tests verifying merge failures route through `OrthoError::Merge`.
//!
//! Ensures deserialization errors during the merge phase (not gathering phase)
//! produce the correct error variant, establishing a clear semantic distinction.

mod fixtures;

use anyhow::{Result, ensure};
use fixtures::merge_fixtures::{MergeErrorSample, VecAppendSample};
use ortho_config::{MergeComposer, OrthoError};
use rstest::rstest;
use serde_json::json;

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

/// Tests that successful vector appending works correctly.
#[rstest]
fn successful_vector_append_produces_correct_result() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "items": [1, 2] }));
    composer.push_environment(json!({ "items": [3, 4] }));

    let config = VecAppendSample::merge_from_layers(composer.layers())?;

    ensure!(
        config.items == vec![1, 2, 3, 4],
        "expected items [1, 2, 3, 4], got {:?}",
        config.items
    );

    Ok(())
}
