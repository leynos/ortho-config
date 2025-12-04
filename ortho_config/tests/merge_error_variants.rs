//! Verifies merge-time failures surface as `OrthoError::Merge`.

use ortho_config::{MergeComposer, OrthoConfig, OrthoError, declarative::LayerComposition};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default, PartialEq, Eq)]
#[ortho_config(prefix = "FAIL_")]
struct MergeFailureConfig {
    #[ortho_config(default = 7)]
    level: u8,
}

#[rstest]
fn merge_from_layers_reports_merge_error() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(ortho_config::serde_json::json!({ "level": 1 }));
    composer.push_cli(ortho_config::serde_json::json!({ "level": "loud" }));

    let err = MergeFailureConfig::merge_from_layers(composer.layers())
        .expect_err("expected merge to fail");

    assert!(matches!(err.as_ref(), OrthoError::Merge { .. }));
}

#[rstest]
fn layer_composition_propagates_merge_error() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(ortho_config::serde_json::json!({ "level": 1 }));
    composer.push_cli(ortho_config::serde_json::json!({ "level": "loud" }));

    let composition = LayerComposition::new(composer.layers(), Vec::new());
    let err = composition
        .into_merge_result(MergeFailureConfig::merge_from_layers)
        .expect_err("expected composed merge to fail");

    assert!(matches!(err.as_ref(), OrthoError::Merge { .. }));
}
