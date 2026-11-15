//! Tests for `[profile.<name>]` table extraction from the file chain:
//! per-file layers, chain order, base-layer stripping (milestone 3).

use camino::Utf8PathBuf;
use pretty_assertions::assert_eq;
use rstest::rstest;
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
    let Some(err) = extract_profile_layers(layers, Some(&selected), false).err() else {
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
    let outcome = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
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
    let outcome = extract_profile_layers(layers, None, false).expect("stripping succeeds");
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
    let err = extract_profile_layers(layers, None, false).expect_err("reserved name must error");
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

/// An invalid profile name is rejected whatever the selection is.
///
/// Validation covers every table in every file layer, so a bad name is a hard
/// error even when the operator selected a different profile, or selected
/// none at all.
#[rstest]
#[case::selection_targets_another_profile(Some("local"))]
#[case::without_a_selection(None)]
fn invalid_name_is_rejected_regardless_of_selection(#[case] selected: Option<&str>) {
    let layers = vec![file_layer(
        json!({ "profile": { "ci!": { "retries": 7 } } }),
        "app.toml",
    )];
    let resolved = selected.map(|name| selection(name).expect("valid test name"));
    let err = extract_profile_layers(layers, resolved.as_ref(), false)
        .expect_err("an invalid name must be rejected regardless of the selection");
    match *err {
        OrthoError::InvalidProfileName { ref name } => assert_eq!(name, "ci!"),
        ref other => panic!("expected InvalidProfileName, got {other:?}"),
    }
}

#[test]
fn forbidden_key_in_a_non_selected_profile_table_is_rejected() {
    let layers = vec![file_layer(
        json!({
            "retries": 3,
            "profile": {
                "ci": { "retries": 7 },
                "staging": { "cmds": { "run": {} } }
            }
        }),
        "app.toml",
    )];
    let err = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect_err("a forbidden key must be rejected in a non-selected table too");
    match *err {
        OrthoError::ProfileForbiddenKey {
            ref profile,
            ref key,
        } => {
            // The error names the offending table, not the selected one.
            assert_eq!(profile, "staging");
            assert_eq!(key, "cmds");
        }
        ref other => panic!("expected ProfileForbiddenKey, got {other:?}"),
    }
}

/// A profile body that is not a table is rejected, selected or not.
///
/// A profile contributes a map of configuration values, so a scalar or array
/// body has no keys to contribute. Left unchecked it would reach the merge as
/// an opaque value that no field can consume, which reads as a type error
/// against an unrelated key rather than as a malformed profile. The rows use a
/// non-selected table as well, so the check cannot be gated on selection.
#[rstest]
#[case::selected_scalar(json!("nope"), true)]
#[case::selected_array(json!([1, 2]), true)]
#[case::non_selected_scalar(json!("nope"), false)]
#[case::non_selected_array(json!([1, 2]), false)]
fn non_table_profile_body_is_rejected(#[case] body: Value, #[case] selected: bool) {
    let profile = if selected { "ci" } else { "staging" };
    let layers = vec![file_layer(
        json!({ "retries": 3, "profile": { "ci": { "retries": 7 }, profile: body } }),
        "app.toml",
    )];
    let err = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect_err("a non-table profile body must be rejected");
    match *err {
        OrthoError::Validation {
            ref key,
            ref message,
        } => {
            assert_eq!(key, &format!("profile.{profile}"));
            assert_eq!(message, "must be an object");
        }
        ref other => panic!("expected Validation, got {other:?}"),
    }
}

/// The selected body is carried through verbatim, whatever it holds.
///
/// The two rows are the two ends of the same contract: profile bodies are not
/// filtered against the struct's fields, so an unrecognized key is preserved
/// for the merge rather than rejected here, and an empty body is a valid no-op
/// rather than an error. Both would be silently broken by validating the
/// body's keys against anything.
#[rstest]
#[case::unknown_key_is_preserved(json!({ "retries": 7, "custom_key": "kept" }))]
#[case::empty_body_is_a_noop(json!({}))]
fn selected_body_flows_through_to_merge_unfiltered(#[case] body: Value) {
    let layers = vec![file_layer(
        json!({ "retries": 3, "profile": { "ci": body.clone() } }),
        "app.toml",
    )];
    let outcome = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect("extraction succeeds");
    let profile_value = outcome
        .profile_layers
        .first()
        .expect("one profile layer")
        .value();
    assert_eq!(profile_value, &body);
}
