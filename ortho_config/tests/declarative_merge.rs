use ortho_config::{MergeLayer, OrthoConfig};
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Deserialize, OrthoConfig, PartialEq, Eq)]
struct DeclarativeConfig {
    recipient: String,
    salutations: Vec<String>,
    #[serde(default)]
    is_excited: bool,
}

#[rstest]
fn merge_layers_respects_precedence() {
    let defaults = MergeLayer::defaults(json!({
        "recipient": "World",
        "salutations": ["Hello"],
        "is_excited": false,
    }));
    let file = MergeLayer::file(
        PathBuf::from("config.toml"),
        json!({
            "salutations": ["Hey", "there"],
        }),
    );
    let env = MergeLayer::environment(json!({ "recipient": "Env" }));
    let cli = MergeLayer::cli(json!({
        "recipient": "Cli",
        "is_excited": true,
    }));

    let merged = DeclarativeConfig::merge_from_layers([defaults, file, env, cli])
        .expect("expected declarative merge to succeed");

    assert_eq!(merged.recipient, "Cli");
    assert_eq!(
        merged.salutations,
        vec!["Hey".to_string(), "there".to_string()]
    );
    assert!(merged.is_excited);
}

#[rstest]
fn merge_layer_requires_object_payload() {
    let result = DeclarativeConfig::merge_from_layers([MergeLayer::defaults(json!("not object"))]);
    let err = result.expect_err("non-object layer must fail");
    assert!(
        err.to_string()
            .contains("defaults layer must be a JSON object"),
        "unexpected error message: {err}"
    );
}

#[rstest]
fn merge_layers_supports_partial_overrides() {
    let defaults = MergeLayer::defaults(json!({
        "recipient": "World",
        "salutations": ["Hello"],
    }));
    let cli = MergeLayer::cli(json!({ "recipient": "Cli" }));

    let merged = DeclarativeConfig::merge_from_layers([defaults, cli])
        .expect("expected partial overrides to succeed");

    assert_eq!(merged.recipient, "Cli");
    assert_eq!(merged.salutations, vec!["Hello".to_string()]);
}
