//! Steps asserting merge failures surface as `OrthoError::Merge`.

use crate::fixtures::{MergeErrorContext, RulesConfig};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MergeComposer, OrthoError};
use rstest_bdd_macros::{given, then, when};

#[given("an invalid CLI merge layer for rules")]
fn invalid_cli_layer(merge_error_context: &MergeErrorContext) -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(ortho_config::serde_json::json!({ "rules": ["ok"] }));
    composer.push_cli(ortho_config::serde_json::json!({ "rules": { "broken": true } }));
    merge_error_context.layers.set(composer.layers());
    Ok(())
}

#[when("the rules configuration is merged")]
fn merge_rules(merge_error_context: &MergeErrorContext) -> Result<()> {
    let layers = merge_error_context
        .layers
        .with_ref(|layers| layers.clone())
        .ok_or_else(|| anyhow!("merge layers missing"))?;
    let result = RulesConfig::merge_from_layers(layers);
    merge_error_context.result.set(result);
    Ok(())
}

#[then("a merge error is returned")]
fn assert_merge_error(merge_error_context: &MergeErrorContext) -> Result<()> {
    let err = merge_error_context
        .result
        .take()
        .ok_or_else(|| anyhow!("merge result missing"))?
        .expect_err("expected merge to fail");
    ensure!(
        matches!(err.as_ref(), OrthoError::Merge { .. }),
        "unexpected error variant: {err:?}"
    );
    Ok(())
}
