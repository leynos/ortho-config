//! Tests for the diagnostic raised when a selected profile cannot be resolved:
//! the structured payload, the discovery-failure distinction, and the
//! capped-and-deduplicated list of names the chain does define.
//!
//! The extraction behaviour these messages accompany is covered by
//! [`super::tests_extraction`]; this module owns only the failure surface.

use camino::Utf8PathBuf;
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use std::borrow::Cow;

use crate::declarative::MergeLayer;
use crate::profile::{ProfileName, ProfileSource, SelectedProfile, extract_profile_layers};

fn file_layer(value: serde_json::Value, path: &str) -> MergeLayer<'static> {
    MergeLayer::file(Cow::Owned(value), Some(Utf8PathBuf::from(path)))
}

/// Builds a flag-selected profile; call sites unwrap inside `#[test]`
/// functions so Whitaker's `no_expect_outside_tests` stays satisfied.
fn selection(name: &str) -> crate::OrthoResult<SelectedProfile> {
    Ok(SelectedProfile {
        name: ProfileName::new(name)?,
        source: ProfileSource::Flag,
    })
}

#[test]
fn unknown_profile_reports_structured_payload() {
    let layers = vec![file_layer(
        serde_json::json!({ "profile": { "local": {}, "ci": { "retries": 7 } } }),
        "app.toml",
    )];
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging").expect("valid test name")),
        false,
    )
    .expect_err("unknown profile must error");
    match *err {
        crate::OrthoError::UnknownProfile {
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
    let err = extract_profile_layers(
        Vec::new(),
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect_err("unknown profile with no files must error");
    let message = err.to_string();
    assert_that!(message, contains_substring("ci"));
    assert_that!(
        message,
        contains_substring("no configuration files were found")
    );
}

/// A discovery failure suppresses the unknown-profile error (the P2 repair).
///
/// The chain is empty because the only candidate failed to parse, so the
/// profile tables were never inspected and no claim about the selection is
/// supportable. The complement — that an empty chain with no discovery failure
/// still reports the unknown profile — is pinned by
/// `no_files_discovered_reports_clear_error` above.
#[test]
fn discovery_failure_suppresses_the_unknown_profile_error() {
    let selected = selection("ci").expect("valid test name");
    let outcome = extract_profile_layers(Vec::new(), Some(&selected), true)
        .expect("a parse error must not be joined by a false selection error");
    assert!(outcome.file_layers.is_empty() && outcome.profile_layers.is_empty());
}

/// A chain that loaded after an unrelated failure is still inspected.
///
/// `discovery_failed` answers "were the profile tables inspected", not "did any
/// candidate fail". Discovery retains the errors of optional candidates it
/// walked past on the way to a later success, so a caller that widened the flag
/// to `!errors.is_empty()` would let a malformed lower-precedence candidate
/// suppress a genuine selection error. Reporting the unknown profile here is
/// the observable difference: the chain the operator named was read, and it
/// does not define `ci`.
#[test]
fn a_chain_that_loaded_after_an_optional_failure_still_reports_the_unknown_profile() {
    let layers = vec![file_layer(serde_json::json!({ "retries": 3 }), "app.toml")];
    let err = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect_err("the surviving chain was inspected, so the selection must be checked");
    let message = err.to_string();
    assert_that!(message, contains_substring("ci"));
    assert_that!(message, contains_substring("no profiles were found"));
}

#[test]
fn chain_without_profile_tables_reports_no_profiles_found() {
    let layers = vec![
        file_layer(serde_json::json!({ "retries": 3 }), "base.toml"),
        file_layer(serde_json::json!({ "retries": 4 }), "app.toml"),
    ];
    let err = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect_err("unknown profile against a real file chain must error");
    let message = err.to_string();
    assert_that!(message, contains_substring("ci"));
    assert_that!(message, contains_substring("no profiles were found"));
    assert_that!(
        message,
        not(contains_substring("no configuration files were found"))
    );
}

#[test]
fn chain_with_an_unrelated_profile_names_that_profile() {
    let layers = vec![file_layer(
        serde_json::json!({ "retries": 3, "profile": { "local": { "retries": 9 } } }),
        "app.toml",
    )];
    let err = extract_profile_layers(
        layers,
        Some(&selection("ci").expect("valid test name")),
        false,
    )
    .expect_err("unknown profile with an unrelated profile defined must error");
    let message = err.to_string();
    // The chain defines a profile, so the message names it instead of
    // claiming no profiles exist.
    assert_that!(message, contains_substring("local"));
    assert_that!(message, not(contains_substring("no profiles were found")));
}

#[test]
fn available_list_display_caps_at_sixteen() {
    let mut profiles = serde_json::Map::new();
    for i in 0..20 {
        profiles.insert(format!("profile_{i}"), serde_json::json!({}));
    }
    let layers = vec![file_layer(
        serde_json::json!({ "profile": profiles }),
        "app.toml",
    )];
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging").expect("valid test name")),
        false,
    )
    .expect_err("unknown profile must error");
    let crate::OrthoError::UnknownProfile { ref available, .. } = *err else {
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
                serde_json::json!({ "profile": { "shared": {} } }),
                &format!("shared-{index}.toml"),
            )
        })
        .collect();
    layers.extend((0..16).map(|index| {
        file_layer(
            serde_json::json!({ "profile": { format!("profile_{index}"): {} } }),
            &format!("unique-{index}.toml"),
        )
    }));
    let err = extract_profile_layers(
        layers,
        Some(&selection("staging").expect("valid test name")),
        false,
    )
    .expect_err("unknown profile must error");
    let crate::OrthoError::UnknownProfile { ref available, .. } = *err else {
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
