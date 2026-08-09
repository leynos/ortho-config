//! Tests for structured profile error paths: unknown, invalid, and reserved
//! names, forbidden keys, and error ordering (milestone 3).

use camino::Utf8PathBuf;
use googletest::prelude::*;
use serde_json::{Value, json};
use std::borrow::Cow;

use crate::declarative::MergeLayer;
use crate::profile::{ProfileName, ProfileSource, SelectedProfile, extract_profile_layers};

fn file_layer(value: Value, path: &str) -> MergeLayer<'static> {
    MergeLayer::file(Cow::Owned(value), Some(Utf8PathBuf::from(path)))
}

fn selection(name: &str, source: ProfileSource) -> SelectedProfile {
    SelectedProfile {
        name: ProfileName::new(name).expect("valid name"),
        source,
    }
}

#[test]
fn forbidden_key_error_display_names_profile_and_key() {
    let layers = vec![file_layer(
        json!({ "profile": { "ci": { "cmds": {} } } }),
        "app.toml",
    )];
    let err = extract_profile_layers(layers, Some(&selection("ci", ProfileSource::Flag)))
        .expect_err("cmds is forbidden");
    let message = err.to_string();
    assert_that!(message, contains_substring("profile 'ci'"));
    assert_that!(message, contains_substring("'cmds'"));
}

#[test]
fn invalid_name_error_display_names_the_grammar() {
    let err = ProfileName::new("bad name").expect_err("invalid name must error");
    let message = err.to_string();
    assert_that!(
        message,
        contains_substring("invalid profile name 'bad name'")
    );
    assert_that!(message, contains_substring("[A-Za-z0-9_-]+"));
}

#[test]
fn reserved_name_error_display_marks_the_name() {
    let err = ProfileName::new("default").expect_err("reserved name must error");
    assert_that!(
        err.to_string(),
        contains_substring("profile name 'default' is reserved")
    );
}

#[test]
fn unknown_profile_display_names_flag_source() {
    let layers = vec![file_layer(json!({ "profile": { "ci": {} } }), "app.toml")];
    let err = extract_profile_layers(layers, Some(&selection("staging", ProfileSource::Flag)))
        .expect_err("unknown profile must error");
    let message = err.to_string();
    assert_that!(message, contains_substring("unknown profile 'staging'"));
    assert_that!(message, contains_substring("via the --profile flag"));
    assert_that!(message, contains_substring("ci"));
}

#[test]
fn unknown_profile_display_names_environment_source() {
    let layers = vec![file_layer(json!({ "profile": { "ci": {} } }), "app.toml")];
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging", ProfileSource::Environment)),
    )
    .expect_err("unknown profile must error");
    assert_that!(
        err.to_string(),
        contains_substring("via the selector environment variable")
    );
}
