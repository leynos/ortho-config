//! CLI metadata wire-compatibility tests.

use super::sample_cli_metadata;
use ortho_config::docs as ortho_docs;

/// IR written before `value_optional` existed still reads as a plain switch.
///
/// The field is additive on the wire but not in meaning: a v1.1 document has no
/// `value_optional` key at all, and the flag it describes is an ordinary
/// switch. Reading it as `true` would render `--flag[=BOOL]` for a flag that
/// rejects values, so the shared schema defaults it to `false`.
#[test]
fn legacy_ir_without_value_optional_defaults_to_false() {
    let legacy = r#"{
        "long": "verbose",
        "short": "v",
        "value_name": null,
        "multiple": false,
        "takes_value": false,
        "possible_values": [],
        "hide_in_help": false
    }"#;

    let from_config: ortho_docs::CliMetadata =
        serde_json::from_str(legacy).expect("legacy IR should read into ortho_config::docs");

    assert!(
        !from_config.value_optional,
        "ortho_config::docs should default a legacy flag to a required value"
    );
    assert!(
        from_config.positional.is_none(),
        "a legacy flag without positional metadata must stay a named option"
    );
}

/// Positional metadata rides an additive optional key on the wire.
#[test]
fn positional_metadata_round_trips_and_is_omitted_when_absent() {
    let flag = serde_json::to_value(sample_cli_metadata()).expect("serialize flag metadata");
    assert!(
        flag.get("positional").is_none(),
        "a named flag must omit the positional key: {flag}"
    );

    let positional = ortho_docs::CliMetadata {
        long: None,
        short: None,
        value_name: Some("PROPOSAL".to_owned()),
        positional: Some(ortho_docs::PositionalMetadata::new(1).with_variadic(true)),
        ..sample_cli_metadata()
    };
    let encoded = serde_json::to_value(&positional).expect("serialize positional metadata");
    assert_eq!(
        encoded
            .get("positional")
            .and_then(|value| value.get("index")),
        Some(&serde_json::Value::from(1)),
        "the positional index must reach the wire: {encoded}"
    );

    let decoded: ortho_docs::CliMetadata =
        serde_json::from_value(encoded).expect("deserialize positional metadata");
    assert_eq!(
        decoded.positional, positional.positional,
        "positional metadata must survive a round trip"
    );
}
