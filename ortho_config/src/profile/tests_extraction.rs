//! Tests for `[profile.<name>]` table extraction from the file chain:
//! per-file layers, chain order, base-layer stripping (milestone 3).

use camino::Utf8PathBuf;
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::borrow::Cow;

use crate::OrthoError;
use crate::OrthoResult;
use crate::declarative::{MergeLayer, MergeProvenance};
use crate::profile::{ProfileName, ProfileSource, SelectedProfile, extract_profile_layers};

fn file_layer(value: Value, path: &str) -> MergeLayer<'static> {
    MergeLayer::file(Cow::Owned(value), Some(Utf8PathBuf::from(path)))
}

/// Builds a flag-selected profile; call sites unwrap inside `#[test]`
/// functions so Whitaker's `no_expect_outside_tests` stays satisfied.
fn selection(name: &str) -> OrthoResult<SelectedProfile> {
    Ok(SelectedProfile {
        name: ProfileName::new(name)?,
        source: ProfileSource::Flag,
    })
}

fn assert_profile_body_key_is_forbidden(expected_key: &str, body: Value) -> OrthoResult<()> {
    let mut profiles = serde_json::Map::new();
    profiles.insert("ci".to_owned(), body);
    let layers = vec![file_layer(json!({ "profile": profiles }), "app.toml")];
    let selected = selection("ci")?;
    let Some(err) = extract_profile_layers(layers, Some(&selected)).err() else {
        return Err(std::sync::Arc::new(OrthoError::Validation {
            key: "profile".to_owned(),
            message: "profile body key unexpectedly succeeded".to_owned(),
        }));
    };
    let OrthoError::ProfileForbiddenKey { profile, key } = err.as_ref() else {
        return Err(err);
    };
    if profile == "ci" && key == expected_key {
        Ok(())
    } else {
        Err(std::sync::Arc::new(OrthoError::Validation {
            key: "profile".to_owned(),
            message: format!(
                "expected forbidden profile key {expected_key:?} for ci, got {key:?} for {profile:?}"
            ),
        }))
    }
}

#[test]
fn extracts_one_profile_layer_per_file_in_chain_order() {
    let layers = vec![
        file_layer(
            json!({ "retries": 3, "profile": { "ci": { "retries": 7 } } }),
            "base.toml",
        ),
        file_layer(
            json!({ "retries": 4, "profile": { "ci": { "retries": 8 } } }),
            "app.toml",
        ),
    ];
    let outcome = extract_profile_layers(layers, Some(&selection("ci").expect("valid test name")))
        .expect("extraction succeeds");
    assert_eq!(outcome.profile_layers.len(), 2);
    let provenances: Vec<MergeProvenance> = outcome
        .profile_layers
        .iter()
        .map(MergeLayer::provenance)
        .collect();
    assert_eq!(
        provenances,
        vec![MergeProvenance::Profile, MergeProvenance::Profile]
    );
    let values: Vec<Value> = outcome
        .profile_layers
        .iter()
        .map(MergeLayer::value)
        .cloned()
        .collect();
    assert_eq!(
        values,
        vec![json!({ "retries": 7 }), json!({ "retries": 8 })]
    );
    let paths: Vec<Option<&str>> = outcome
        .profile_layers
        .iter()
        .map(|layer| layer.path().map(camino::Utf8Path::as_str))
        .collect();
    assert_eq!(paths, vec![Some("base.toml"), Some("app.toml")]);
}

#[test]
fn strips_profile_key_from_file_layers_even_without_selection() {
    let layers = vec![file_layer(
        json!({ "retries": 3, "profile": { "ci": { "retries": 7 } } }),
        "app.toml",
    )];
    let outcome = extract_profile_layers(layers, None).expect("stripping succeeds");
    assert_eq!(outcome.profile_layers.len(), 0);
    let stripped = outcome.file_layers.first().expect("one file layer");
    assert_eq!(stripped.value(), &json!({ "retries": 3 }));
    assert_eq!(stripped.provenance(), MergeProvenance::File);
}

#[test]
fn defining_default_profile_is_an_error() {
    let layers = vec![file_layer(
        json!({ "profile": { "default": { "retries": 7 } } }),
        "app.toml",
    )];
    let err = extract_profile_layers(layers, None).expect_err("reserved name must error");
    assert!(matches!(*err, OrthoError::ReservedProfileName { .. }));
}

#[test]
fn inherits_key_inside_profile_body_is_forbidden() {
    assert_profile_body_key_is_forbidden("inherits", json!({ "inherits": "base", "retries": 7 }))
        .expect("inherits profile body key is forbidden");
}

#[test]
fn cmds_key_inside_profile_body_is_forbidden() {
    assert_profile_body_key_is_forbidden("cmds", json!({ "cmds": { "run": {} } }))
        .expect("cmds profile body key is forbidden");
}

#[test]
fn unknown_profile_reports_structured_payload() {
    let layers = vec![file_layer(
        json!({ "profile": { "local": {}, "ci": { "retries": 7 } } }),
        "app.toml",
    )];
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging").expect("valid test name")),
    )
    .expect_err("unknown profile must error");
    match *err {
        OrthoError::UnknownProfile {
            ref selected,
            ref selection_source,
            ref available,
        } => {
            assert_eq!(selected, "staging");
            assert_eq!(*selection_source, ProfileSource::Flag);
            let expected = vec!["ci".to_owned(), "local".to_owned()];
            assert_eq!(available.as_slice(), expected.as_slice());
        }
        ref other => panic!("expected UnknownProfile, got {other:?}"),
    }
}

#[test]
fn no_files_discovered_reports_clear_error() {
    let err = extract_profile_layers(Vec::new(), Some(&selection("ci").expect("valid test name")))
        .expect_err("unknown profile with no files must error");
    let message = err.to_string();
    assert_that!(message, contains_substring("ci"));
    assert_that!(
        message,
        contains_substring("no configuration files were found")
    );
}

#[test]
fn unknown_profile_body_keys_flow_through_to_merge() {
    let layers = vec![file_layer(
        json!({
            "retries": 3,
            "profile": { "ci": { "retries": 7, "custom_key": "kept" } }
        }),
        "app.toml",
    )];
    let outcome = extract_profile_layers(layers, Some(&selection("ci").expect("valid test name")))
        .expect("extraction succeeds");
    let profile_value = outcome
        .profile_layers
        .first()
        .expect("one profile layer")
        .value();
    assert_eq!(
        profile_value,
        &json!({ "retries": 7, "custom_key": "kept" })
    );
}

#[test]
fn empty_profile_table_is_a_valid_noop() {
    let layers = vec![file_layer(
        json!({ "retries": 3, "profile": { "ci": {} } }),
        "app.toml",
    )];
    let outcome = extract_profile_layers(layers, Some(&selection("ci").expect("valid test name")))
        .expect("extraction succeeds");
    let profile_value = outcome
        .profile_layers
        .first()
        .expect("one profile layer")
        .value();
    assert_eq!(profile_value, &json!({}));
}

#[test]
fn available_list_display_caps_at_sixteen() {
    let mut profiles = serde_json::Map::new();
    for i in 0..20 {
        profiles.insert(format!("profile_{i}"), json!({}));
    }
    let layers = vec![file_layer(json!({ "profile": profiles }), "app.toml")];
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging").expect("valid test name")),
    )
    .expect_err("unknown profile must error");
    let OrthoError::UnknownProfile { ref available, .. } = *err else {
        panic!("expected UnknownProfile error");
    };
    assert_eq!(available.as_slice().len(), 16);
    let message = err.to_string();
    assert_that!(message, contains_substring("and 4 more"));
    assert_that!(message, contains_substring("profile_0"));
    assert_that!(message, contains_substring("profile_15"));
}

#[test]
fn available_list_deduplicates_before_capping() {
    let mut layers: Vec<_> = (0..20)
        .map(|index| {
            file_layer(
                json!({ "profile": { "shared": {} } }),
                &format!("shared-{index}.toml"),
            )
        })
        .collect();
    layers.extend((0..16).map(|index| {
        file_layer(
            json!({ "profile": { format!("profile_{index}"): {} } }),
            &format!("unique-{index}.toml"),
        )
    }));
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging").expect("valid test name")),
    )
    .expect_err("unknown profile must error");
    let OrthoError::UnknownProfile { ref available, .. } = *err else {
        panic!("expected UnknownProfile error");
    };
    assert_eq!(available.as_slice().len(), 16);
    assert_eq!(
        available
            .as_slice()
            .iter()
            .filter(|name| *name == "shared")
            .count(),
        0
    );
    assert_that!(err.to_string(), contains_substring("and 1 more"));
}
