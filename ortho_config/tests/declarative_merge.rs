//! Integration tests for declarative configuration merging.
//!
//! Validates layer composition, precedence, append strategies for Vec fields,
//! and Option null handling in the declarative merge system.
use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::{
    MergeComposer, MergeLayer, MergeProvenance, OrthoConfig, declarative::merge_value,
};
use rstest::{fixture, rstest};
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
    cli_layer: Option<serde_json::Value>,
) -> Vec<MergeLayer<'static>> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_environment(environment);
    if let Some(layer) = cli_layer {
        composer.push_cli(layer);
    }
    composer.layers()
}

#[fixture]
fn precedence_defaults() -> serde_json::Value {
    json!({
        "name": "Default",
        "count": 1,
        "flag": false,
    })
}

#[derive(Clone, Copy)]
struct ExpectedDeclarativeSample {
    name: &'static str,
    count: u32,
    flag: bool,
}

const fn expected_sample(name: &'static str, count: u32, flag: bool) -> ExpectedDeclarativeSample {
    ExpectedDeclarativeSample { name, count, flag }
}

struct PrecedenceScenario {
    file: Option<serde_json::Value>,
    environment: Option<serde_json::Value>,
    cli: Option<serde_json::Value>,
    expected: ExpectedDeclarativeSample,
}

const fn precedence_case(
    file: Option<serde_json::Value>,
    environment: Option<serde_json::Value>,
    cli: Option<serde_json::Value>,
    expected: ExpectedDeclarativeSample,
) -> PrecedenceScenario {
    PrecedenceScenario {
        file,
        environment,
        cli,
        expected,
    }
}

#[rstest]
#[case::defaults_only(precedence_case(None, None, None, expected_sample("Default", 1, false)))]
#[case::file_only(precedence_case(
    Some(json!({"name": "File", "count": 2})),
    None,
    None,
    expected_sample("File", 2, false),
))]
#[case::environment_only(precedence_case(
    None,
    Some(json!({"name": "Env", "count": 3, "flag": true})),
    None,
    expected_sample("Env", 3, true),
))]
#[case::cli_only(precedence_case(
    None,
    None,
    Some(json!({"name": "Cli", "flag": true})),
    expected_sample("Cli", 1, true),
))]
#[case::environment_over_file(precedence_case(
    Some(json!({"name": "File", "count": 4})),
    Some(json!({"name": "Env", "count": 6})),
    None,
    expected_sample("Env", 6, false),
))]
#[case::cli_overrides_file(precedence_case(
    Some(json!({"name": "File", "count": 2, "flag": true})),
    None,
    Some(json!({"name": "Cli"})),
    expected_sample("Cli", 2, true),
))]
#[case::cli_overrides_environment(precedence_case(
    None,
    Some(json!({"name": "Env", "count": 5, "flag": true})),
    Some(json!({"count": 9})),
    expected_sample("Env", 9, true),
))]
#[case::all_layers(precedence_case(
    Some(json!({"name": "File", "count": 2, "flag": true})),
    Some(json!({"name": "Env", "count": 7})),
    Some(json!({"name": "Cli", "flag": false})),
    expected_sample("Cli", 7, false),
))]
fn merge_layers_respect_precedence_permutations(
    precedence_defaults: serde_json::Value,
    #[case] scenario: PrecedenceScenario,
) -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(precedence_defaults);
    if let Some(file_value) = scenario.file {
        composer.push_file(file_value, Some(Utf8PathBuf::from("config.json")));
    }
    if let Some(environment_value) = scenario.environment {
        composer.push_environment(environment_value);
    }
    if let Some(cli_value) = scenario.cli {
        composer.push_cli(cli_value);
    }

    let config = to_anyhow(DeclarativeSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.name == scenario.expected.name,
        "expected name {} but observed {}",
        scenario.expected.name,
        config.name,
    );
    ensure!(
        config.count == scenario.expected.count,
        "expected count {} but observed {}",
        scenario.expected.count,
        config.count,
    );
    ensure!(
        config.flag == scenario.expected.flag,
        "expected flag {} but observed {}",
        scenario.expected.flag,
        config.flag,
    );
    Ok(())
}

fn to_anyhow<T, E: std::fmt::Display>(result: Result<T, E>) -> anyhow::Result<T> {
    result.map_err(|err| anyhow!(err.to_string()))
}

#[rstest]
#[case::cli_overrides(json!({"name": "default", "count": 1, "flag": false }), json!({}), json!({"count": 5}), 5)]
#[case::env_over_defaults(json!({"name": "default", "count": 1, "flag": false }), json!({"count": 3}), json!({}), 3)]
fn merge_layers_respect_precedence(
    #[case] defaults: serde_json::Value,
    #[case] environment: serde_json::Value,
    #[case] cli: serde_json::Value,
    #[case] expected_count: u32,
) -> Result<()> {
    let layers = compose_layers(defaults, environment, Some(cli));
    let config = to_anyhow(DeclarativeSample::merge_from_layers(layers))?;
    ensure!(
        config.count == expected_count,
        "expected {expected_count}, got {}",
        config.count
    );
    ensure!(
        config.name == "default",
        "expected name default, got {}",
        config.name
    );
    ensure!(!config.flag, "expected flag false, got true");
    Ok(())
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
fn merge_value_nullifies_fields() {
    let mut target = json!({ "num": 1, "str": "foo", "bool": true });
    let layer = json!({ "num": null, "str": null, "bool": null });
    merge_value(&mut target, layer);
    assert_eq!(target, json!({ "num": null, "str": null, "bool": null }));
}

#[rstest]
fn merge_value_overwrites_null_fields() {
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
fn merge_layers_append_vectors() -> Result<()> {
    let layers = compose_layers(
        json!({ "values": ["default"] }),
        json!({ "values": ["env"] }),
        Some(json!({ "values": ["cli"] })),
    );
    let config = to_anyhow(AppendSample::merge_from_layers(layers))?;
    let expected = vec![
        String::from("default"),
        String::from("env"),
        String::from("cli"),
    ];
    ensure!(
        config.values == expected,
        "vectors accumulate incorrectly: expected {:?}, got {:?}",
        expected,
        config.values
    );
    Ok(())
}

#[rstest]
fn merge_layers_respect_option_nulls() -> Result<()> {
    let layers = compose_layers(json!({ "flag": "present" }), json!({ "flag": null }), None);
    let config = to_anyhow(OptionalSample::merge_from_layers(layers))?;
    ensure!(
        config.flag.is_none(),
        "expected flag to be None, got {:?}",
        config.flag
    );
    Ok(())
}
#[rstest]
fn merge_from_layers_accepts_file_layers() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "name": "default", "count": 1, "flag": false }));
    composer.push_file(
        json!({ "name": "from_file", "count": 7, "flag": true }),
        Some(Utf8PathBuf::from("config.json")),
    );
    let config = to_anyhow(DeclarativeSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.name == "from_file",
        "expected name from_file, got {}",
        config.name
    );
    ensure!(config.count == 7, "expected count 7, got {}", config.count);
    ensure!(config.flag, "expected flag true");
    Ok(())
}

#[rstest]
fn merge_layers_reject_non_object_values() -> Result<()> {
    let layers = compose_layers(
        json!({ "name": "default", "count": 1, "flag": false }),
        json!(true),
        None,
    );

    let error = match DeclarativeSample::merge_from_layers(layers) {
        Ok(config) => return Err(anyhow!("expected merge failure, got config {config:?}")),
        Err(err) => err,
    };
    let message = error.to_string();
    ensure!(
        message.contains("expects JSON objects"),
        "unexpected error message: {message}"
    );
    ensure!(
        message.contains("environment layer supplied a boolean"),
        "missing provenance context: {message}"
    );
    Ok(())
}
