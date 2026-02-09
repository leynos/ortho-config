//! Steps verifying merge error routing to `OrthoError::Merge`.

use crate::fixtures::{MergeErrorContext, MergeErrorSample};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MergeComposer, OrthoError};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

#[given("a merge layer with port set to {value}")]
fn layer_with_port(merge_error_context: &MergeErrorContext, value: String) -> Result<()> {
    let layer_value = if let Some(inner) = value.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
    {
        // String value like "not_a_number"
        json!({ "port": inner })
    } else {
        // Numeric value
        let num: u16 = value.parse().map_err(|e| anyhow!("invalid port: {e}"))?;
        json!({ "port": num })
    };

    let mut composer = MergeComposer::new();
    composer.push_defaults(layer_value);
    merge_error_context.layers.set(composer.layers());
    Ok(())
}

#[when("the layers are merged")]
fn merge_layers(merge_error_context: &MergeErrorContext) -> Result<()> {
    let layers = merge_error_context
        .layers
        .take()
        .ok_or_else(|| anyhow!("layers not initialised"))?;
    let result = MergeErrorSample::merge_from_layers(layers);
    merge_error_context.result.set(result);
    Ok(())
}

#[then("a Merge error variant is returned")]
fn expect_merge_error(merge_error_context: &MergeErrorContext) -> Result<()> {
    let result = merge_error_context
        .result
        .take()
        .ok_or_else(|| anyhow!("merge result unavailable"))?;
    let err = result.err().ok_or_else(|| anyhow!("expected error"))?;

    ensure!(
        matches!(&*err, OrthoError::Merge { .. }),
        "expected Merge error variant, got {err:?}"
    );
    Ok(())
}

#[then("the merged config has port {expected}")]
fn expect_port(merge_error_context: &MergeErrorContext, expected: u16) -> Result<()> {
    let result = merge_error_context
        .result
        .take()
        .ok_or_else(|| anyhow!("merge result unavailable"))?;
    let config = result.map_err(|e| anyhow!("merge failed: {e}"))?;

    ensure!(
        config.port == expected,
        "expected port {expected}, got {}",
        config.port
    );
    Ok(())
}
