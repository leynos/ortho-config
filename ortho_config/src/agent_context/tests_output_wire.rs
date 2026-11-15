//! Tolerant decoding of structured-output contracts in agent context.
//!
//! The wire format is the lenient half of the contract. ADR-0023 gives it two
//! obligations this module pins: an unrecognized stdout token reads as
//! `unknown` without disturbing the rest of the document, an undeclared member
//! is omitted rather than written as `null`, and a present `null` is rejected
//! so absence keeps exactly one spelling. The strict authoring
//! schema, which rejects both of those inputs, is covered by
//! `ortho_config/tests/docs_ir_output.rs`.
//!
//! `unknown` is a fifth, explicit state. It never establishes data-returning
//! status, so no test here treats it as interchangeable with `data`.

use rstest::rstest;
use serde_json::{Value, json};

use super::*;

/// Decodes a context holding the supplied command array, keeping the error.
fn try_decode_context(commands: &Value) -> Result<AgentContext, serde_json::Error> {
    serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "wire-cli.agent_context",
        "package": "wire-cli",
        "commands": commands
    }))
}

/// Decodes a context holding the supplied command array.
fn decode_context(commands: &Value) -> AgentContext {
    try_decode_context(commands).expect("the fixture context should decode")
}

/// Decodes a one-command context and returns that command's contract.
fn decode_output(output: &Value) -> OutputContract {
    let context = decode_context(&json!([{"path": ["wire-cli"], "output": output}]));
    context
        .commands
        .first()
        .expect("the fixture context should contain one command")
        .output
        .clone()
        .expect("the fixture command should declare an output contract")
}

/// Omission is the only spelling of an absent contract.
#[test]
fn omitted_output_stays_none() {
    let context = decode_context(&json!([{"path": ["wire-cli"]}]));

    assert_eq!(
        context
            .commands
            .first()
            .and_then(|declared| declared.output.as_ref()),
        None,
        "an omitted block must not become a default contract"
    );
}

/// A present `null` is invalid for the block and for every optional member.
///
/// Without this rejection `null` would decode to `None` through `Option`,
/// making a decode accident look like an author who declared nothing, which
/// ADR-0022 forbids inside agent context.
#[rstest]
#[case::block(json!({"path": ["wire-cli"], "output": null}), "the output block")]
#[case::supports_json(
    json!({"path": ["wire-cli"], "output": {"supports_json": null}}),
    "supports_json"
)]
#[case::stdout(
    json!({"path": ["wire-cli"], "output": {"stdout": null}}),
    "stdout"
)]
#[case::stderr_diagnostics(
    json!({"path": ["wire-cli"], "output": {"stderr_diagnostics": null}}),
    "stderr_diagnostics"
)]
fn present_null_is_rejected(#[case] command: Value, #[case] member: &str) {
    let decoded = try_decode_context(&json!([command]));

    assert!(
        decoded.is_err(),
        "a present null for {member} must be rejected, not read as undeclared"
    );
}

/// An empty block is present but claims nothing.
#[test]
fn empty_output_block_claims_nothing() {
    let output = decode_output(&json!({}));

    assert_eq!(
        output,
        OutputContract::default(),
        "an empty block should decode to the empty contract"
    );
}

/// Every accepted token decodes to its own variant.
#[rstest]
#[case::data("data", StdoutPurpose::Data)]
#[case::path("path", StdoutPurpose::Path)]
#[case::summary("summary", StdoutPurpose::Summary)]
#[case::none("none", StdoutPurpose::None)]
#[case::unknown("unknown", StdoutPurpose::Unknown)]
fn known_stdout_tokens_decode(#[case] token: &str, #[case] expected: StdoutPurpose) {
    let output = decode_output(&json!({"stdout": token}));

    assert_eq!(
        output.stdout,
        Some(expected),
        "token {token:?} should decode to {expected:?}"
    );
}

/// A future token normalizes instead of failing the whole document.
#[test]
fn future_stdout_token_normalizes_to_unknown() {
    let output = decode_output(&json!({"stdout": "structured_stream"}));

    assert_eq!(
        output.stdout,
        Some(StdoutPurpose::Unknown),
        "an unrecognized token should read as unknown"
    );
    assert_ne!(
        output.stdout,
        Some(StdoutPurpose::Data),
        "unknown must never establish data-returning status"
    );
}

/// `unknown` is a stable wire token, so a decoded fallback re-encodes as
/// `unknown` rather than vanishing or reverting to a concrete purpose.
#[test]
fn unknown_serializes_as_the_unknown_token() {
    let contract = OutputContract {
        stdout: Some(StdoutPurpose::Unknown),
        ..OutputContract::default()
    };
    let encoded = serde_json::to_value(&contract).expect("the contract should encode");

    assert_eq!(
        encoded,
        json!({"stdout": "unknown"}),
        "the fallback needs its own stable spelling"
    );
}

/// An unrecognized member inside the block is ignored, not fatal.
#[test]
fn future_members_are_tolerated() {
    let output = decode_output(&json!({
        "supports_json": true,
        "schema_reference": {"kind": "response", "uri": "schema.json"}
    }));

    assert_eq!(
        output.supports_json,
        Some(true),
        "a future member must not discard the declared leaves"
    );
    assert_eq!(
        output.stdout, None,
        "a future member must not invent a stdout purpose"
    );
}

/// Declared false survives, so refusal and silence stay distinguishable.
#[test]
fn declared_refusal_survives() {
    let output = decode_output(&json!({"supports_json": false, "stderr_diagnostics": false}));

    assert_eq!(
        output.supports_json,
        Some(false),
        "a refusal should survive decoding"
    );
    assert_eq!(
        output.stderr_diagnostics,
        Some(false),
        "declared diagnostic stderr should survive decoding"
    );
    assert_ne!(
        output.supports_json, None,
        "a refusal is not the same declaration as silence"
    );
}

/// A `null` class vector is malformed rather than an empty declaration.
#[test]
fn null_class_vector_is_rejected() {
    let decoded: Result<AgentContext, _> = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "wire-cli.agent_context",
        "package": "wire-cli",
        "commands": [{"path": ["wire-cli"], "output": {"exit_classes": null}}]
    }));

    assert!(
        decoded.is_err(),
        "a null list is a type error, not an absence"
    );
}

/// Absent members are omitted on the wire, and class order is preserved.
#[test]
fn serialization_omits_absent_members_in_author_order() {
    let contract = OutputContract {
        supports_json: Some(true),
        stdout: None,
        stderr_diagnostics: None,
        exit_classes: vec![
            ExitClass {
                name: "usage".to_owned(),
                meaning: "The invocation was invalid".to_owned(),
            },
            ExitClass {
                name: "success".to_owned(),
                meaning: "The command completed".to_owned(),
            },
        ],
    };
    let encoded = serde_json::to_value(&contract).expect("the contract should encode");

    assert_eq!(
        encoded,
        json!({
            "supports_json": true,
            "exit_classes": [
                {"name": "usage", "meaning": "The invocation was invalid"},
                {"name": "success", "meaning": "The command completed"}
            ]
        }),
        "absent members should be omitted and order preserved"
    );
}

/// The round trip preserves every declared member.
#[test]
fn declared_contract_round_trips() {
    let declared = json!({
        "supports_json": true,
        "stdout": "path",
        "stderr_diagnostics": true,
        "exit_classes": [{"name": "success", "meaning": "The command completed"}]
    });
    let output = decode_output(&declared);

    assert_eq!(
        serde_json::to_value(&output).expect("the decoded contract should encode"),
        declared,
        "a fully declared contract should round trip unchanged"
    );
}
