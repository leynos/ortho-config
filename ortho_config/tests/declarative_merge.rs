use ortho_config::{
    MergeComposer, MergeLayer, MergeProvenance, OrthoConfig, declarative::merge_value,
};
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, OrthoConfig)]
struct DeclarativeSample {
    name: String,
    count: u32,
    flag: bool,
}

fn compose_layers(
    defaults: serde_json::Value,
    environment: serde_json::Value,
    cli: serde_json::Value,
) -> Vec<MergeLayer<'static>> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_environment(environment);
    composer.push_cli(cli);
    composer.layers()
}

#[rstest]
#[case::cli_overrides(json!({"name": "default", "count": 1, "flag": false }), json!({}), json!({"count": 5}), 5)]
#[case::env_over_defaults(json!({"name": "default", "count": 1, "flag": false }), json!({"count": 3}), json!({}), 3)]
fn merge_layers_respect_precedence(
    #[case] defaults: serde_json::Value,
    #[case] environment: serde_json::Value,
    #[case] cli: serde_json::Value,
    #[case] expected_count: u32,
) {
    let layers = compose_layers(defaults, environment, cli);
    let config = DeclarativeSample::merge_from_layers(layers).expect("merge succeeds");
    assert_eq!(config.count, expected_count);
    assert_eq!(config.name, "default");
    assert!(!config.flag);
}

#[rstest]
fn merge_layer_preserves_provenance() {
    let layer = MergeLayer::environment(std::borrow::Cow::Owned(json!({"key": "value"})));
    assert_eq!(layer.provenance(), MergeProvenance::Environment);
    assert!(layer.path().is_none());
}

#[rstest]
fn merge_value_merges_nested_objects() {
    let mut target = json!({ "outer": { "inner": false } });
    let layer = json!({ "outer": { "inner": true } });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "outer": { "inner": true } }));
}
