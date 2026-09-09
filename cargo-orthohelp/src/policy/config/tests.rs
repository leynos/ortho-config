//! Tests for the policy configuration model and Cargo-metadata parsing.
//!
//! The metadata fragments mirror the JSON shape Cargo produces for
//! `package.metadata` when it converts `[package.metadata.ortho_config.policy]`
//! from `Cargo.toml`; the tool never sees the TOML text directly.

use super::*;
use proptest::{collection::vec, option, prelude::*};
use rstest::rstest;
use serde_json::{Value, json};

use crate::policy::{PolicyResult, PolicySeverity, PolicySummary};

fn parse_policy(metadata: &Value) -> Result<Option<PolicyConfigMetadata>, serde_json::Error> {
    PolicyConfigMetadata::from_package_metadata(metadata)
}

#[rstest]
fn absent_policy_table_yields_none() {
    let parsed = parse_policy(&json!({})).expect("metadata without ortho_config should parse");
    assert!(parsed.is_none());
}

#[rstest]
fn absent_ortho_config_table_yields_none() {
    let parsed = parse_policy(&json!({ "root_type": "demo::Config" }))
        .expect("metadata without ortho_config.policy should parse");
    assert!(parsed.is_none());
}

#[rstest]
fn empty_policy_table_defaults_to_off() {
    let parsed = parse_policy(&json!({ "ortho_config": { "policy": {} } }))
        .expect("empty policy table should parse")
        .expect("policy table should be present");
    assert_eq!(parsed.mode, PolicyMode::Off);
    assert!(parsed.exceptions.is_empty());
}

#[rstest]
#[case("off", PolicyMode::Off)]
#[case("warn", PolicyMode::Warn)]
#[case("deny", PolicyMode::Deny)]
fn parses_each_mode(#[case] wire: &str, #[case] expected: PolicyMode) {
    let parsed = parse_policy(&json!({ "ortho_config": { "policy": { "mode": wire } } }))
        .expect("mode should parse")
        .expect("policy table should be present");
    assert_eq!(parsed.mode, expected);
}

#[rstest]
fn parses_exception_with_reason_and_command_path() {
    let parsed = parse_policy(&json!({
        "ortho_config": {
            "policy": {
                "mode": "warn",
                "exceptions": [
                    { "kind": "flag", "name": "--format", "reason": "legacy flag", "command_path": "fixture" }
                ]
            }
        }
    }))
    .expect("policy should parse")
    .expect("policy table should be present");

    assert_eq!(
        parsed.exceptions,
        [PolicyException {
            kind: ExceptionKind::Flag,
            name: "--format".to_owned(),
            reason: "legacy flag".to_owned(),
            command_path: Some("fixture".to_owned()),
        }]
    );
}

#[rstest]
fn parses_exception_without_command_path() {
    let parsed = parse_policy(&json!({
        "ortho_config": {
            "policy": {
                "exceptions": [
                    { "kind": "verb", "name": "get", "reason": "migration" }
                ]
            }
        }
    }))
    .expect("policy should parse")
    .expect("policy table should be present");

    let exception = parsed
        .exceptions
        .first()
        .expect("policy should contain one exception");
    assert_eq!(exception.command_path, None);
}

#[rstest]
#[case(
    json!({
        "ortho_config": {
            "policy": {
                "exceptions": [
                    { "kind": "verb", "name": "get" }
                ]
            }
        }
    }),
    "exception without reason"
)]
#[case(
    json!({
        "ortho_config": {
            "policy": {
                "mode": "warn",
                "tolerance": "high"
            }
        }
    }),
    "unknown policy table key"
)]
#[case(
    json!({
        "ortho_config": {
            "policy": {
                "exceptions": [
                    { "kind": "verb", "name": "get", "reason": "r", "scope": "global" }
                ]
            }
        }
    }),
    "unknown exception key"
)]
fn invalid_policy_metadata_is_an_error(#[case] metadata: Value, #[case] description: &str) {
    let error =
        parse_policy(&metadata).expect_err("invalid policy metadata should fail deserialization");

    assert!(
        error.is_data() || error.is_syntax(),
        "expected a data or syntax error for '{description}', got {error}"
    );
}

#[rstest]
fn mode_defaults_to_off_when_table_omits_it() {
    let parsed = parse_policy(&json!({
        "ortho_config": {
            "policy": {
                "exceptions": []
            }
        }
    }))
    .expect("policy without mode should parse")
    .expect("policy table should be present");

    assert_eq!(parsed.mode, PolicyMode::Off);
}

#[rstest]
fn resolved_config_apply_override_and_defaults() {
    let metadata = PolicyConfigMetadata {
        mode: PolicyMode::Warn,
        exceptions: Vec::new(),
    };
    let config = PolicyConfig::from(&metadata);
    assert_eq!(config.mode, PolicyMode::Warn);

    let default_config = PolicyConfig::default();
    assert_eq!(default_config.mode, PolicyMode::Off);
    assert!(default_config.exceptions.is_empty());
}

proptest! {
    #[test]
    fn policy_config_metadata_round_trips(config in any_policy_config_metadata()) {
        let value = serde_json::to_value(&config).expect("serialize policy config");
        let parsed: PolicyConfigMetadata =
            serde_json::from_value(value).expect("parse policy config");
        prop_assert_eq!(parsed, config);
    }

    #[test]
    fn summary_totals_match_severity_counts(results in vec(any_policy_result(), 0..8)) {
        let summary = PolicySummary::from_results(&results);
        prop_assert_eq!(summary.total, summary.off + summary.warn + summary.deny);
        prop_assert_eq!(summary.total, results.len());
    }
}

fn any_policy_config_metadata() -> impl Strategy<Value = PolicyConfigMetadata> {
    (any_policy_mode(), vec(any_policy_exception(), 0..3))
        .prop_map(|(mode, exceptions)| PolicyConfigMetadata { mode, exceptions })
}

fn any_policy_mode() -> impl Strategy<Value = PolicyMode> {
    prop_oneof![
        Just(PolicyMode::Off),
        Just(PolicyMode::Warn),
        Just(PolicyMode::Deny),
    ]
}

fn any_policy_exception() -> impl Strategy<Value = PolicyException> {
    (
        any_exception_kind(),
        "[a-z][a-z0-9-]{0,16}",
        "[A-Za-z0-9 .,;-]{0,48}",
        option::of("[a-z][a-z0-9 -]{0,12}"),
    )
        .prop_map(|(kind, name, reason, command_path)| PolicyException {
            kind,
            name,
            reason,
            command_path,
        })
}

fn any_exception_kind() -> impl Strategy<Value = ExceptionKind> {
    prop_oneof![Just(ExceptionKind::Verb), Just(ExceptionKind::Flag)]
}

fn any_policy_result() -> impl Strategy<Value = PolicyResult> {
    (
        "[a-z][a-z0-9.-]{0,32}",
        "[a-z][a-z0-9_]{0,32}",
        any_policy_severity(),
        "[A-Za-z0-9 .,;-]{0,64}",
    )
        .prop_map(|(rule_id, code, severity, message)| PolicyResult {
            rule_id,
            code,
            severity,
            message,
            location: None,
        })
}

fn any_policy_severity() -> impl Strategy<Value = PolicySeverity> {
    prop_oneof![
        Just(PolicySeverity::Off),
        Just(PolicySeverity::Warn),
        Just(PolicySeverity::Deny),
    ]
}
