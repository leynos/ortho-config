//! Integration tests for declarative configuration merging.
//!
//! Validates layer composition, precedence, append strategies for Vec fields,
//! and Option null handling in the declarative merge system.

use camino::Utf8PathBuf;
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

#[derive(Debug, Deserialize, OrthoConfig)]
struct AppendSample {
    values: Vec<String>,
}

#[derive(Debug, Deserialize, OrthoConfig)]
struct OptionalSample {
    flag: Option<String>,
}

fn compose_layers(
    defaults: serde_json::Value,
    environment: serde_json::Value,
    cli: Option<serde_json::Value>,
) -> Vec<MergeLayer<'static>> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_environment(environment);
    if let Some(cli) = cli {
        composer.push_cli(cli);
    }
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
    let layers = compose_layers(defaults, environment, Some(cli));
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

#[rstest]
fn merge_value_merges_arrays() {
    let mut target = json!({ "arr": [1, 2, 3] });
    let layer = json!({ "arr": [4, 5] });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "arr": [4, 5] }));
}

#[rstest]
fn merge_value_merges_scalars() {
    let mut target = json!({ "num": 1, "str": "foo", "bool": false });
    let layer = json!({ "num": 42, "str": "bar", "bool": true });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "num": 42, "str": "bar", "bool": true }));
}

#[rstest]
fn merge_value_merges_null_values() {
    let mut target = json!({ "num": 1, "str": "foo", "bool": true });
    let layer = json!({ "num": null, "str": null, "bool": null });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "num": null, "str": null, "bool": null }));

    let mut target = json!({ "num": null, "str": null, "bool": null });
    let layer = json!({ "num": 99, "str": "baz", "bool": false });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "num": 99, "str": "baz", "bool": false }));
}

#[rstest]
fn merge_value_merges_absent_target_key() {
    let mut target = json!({});
    let layer = json!({ "new_key": "new_value", "another": 123 });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "new_key": "new_value", "another": 123 }));
}

#[rstest]
fn merge_layers_append_vectors() {
    let layers = compose_layers(
        json!({ "values": ["default"] }),
        json!({ "values": ["env"] }),
        Some(json!({ "values": ["cli"] })),
    );
    let config = AppendSample::merge_from_layers(layers).expect("merge succeeds");
    assert_eq!(
        config.values,
        vec![
            String::from("default"),
            String::from("env"),
            String::from("cli"),
        ],
        "vectors accumulate in defaults, environment, CLI order"
    );
}

#[rstest]
fn merge_layers_respect_option_nulls() {
    let layers = compose_layers(
        json!({ "flag": "present" }),
        json!({ "flag": null }),
        None,
    );
    let config = OptionalSample::merge_from_layers(layers).expect("merge succeeds");
    assert!(config.flag.is_none());
}
#[rstest]
fn merge_from_layers_accepts_file_layers() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "name": "default", "count": 1, "flag": false }));
    composer.push_file(
        json!({ "name": "from_file", "count": 7, "flag": true }),
        Some(Utf8PathBuf::from("config.json")),
    );
    let config = DeclarativeSample::merge_from_layers(composer.layers()).expect("merge succeeds");
    assert_eq!(config.name, "from_file");
    assert_eq!(config.count, 7);
    assert!(config.flag);
}

#[rstest]
fn merge_layers_reject_non_object_values() {
    let layers = compose_layers(
        json!({ "name": "default", "count": 1, "flag": false }),
        json!(true),
        None,
    );

    let error =
        DeclarativeSample::merge_from_layers(layers).expect_err("non-object layer is rejected");
    let message = error.to_string();
    assert!(
        message.contains("expects JSON objects"),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("environment layer supplied a boolean"),
        "missing provenance context: {message}"
    );
}
