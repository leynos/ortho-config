//! Tolerant decoding of structured-output contracts in agent context.
//!
//! The wire format is the lenient half of the contract. ADR-0023 gives it two
//! obligations this module pins: an unrecognized stdout token reads as
//! `unknown` without disturbing the rest of the document, and an undeclared
//! member is omitted rather than written as `null`. The strict authoring
//! schema, which rejects both of those inputs, is covered by
//! `ortho_config/tests/docs_ir_output.rs`.
//!
//! `unknown` is a fifth, explicit state. It never establishes data-returning
//! status, so no test here treats it as interchangeable with `data`.

use rstest::rstest;
use serde_json::{Value, json};

use super::*;

/// Decodes a one-command context whose command carries `output`.
fn decode_output(output: Value) -> Result<OutputContract, serde_json::Error> {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "wire-cli.agent_context",
        "package": "wire-cli",
        "commands": [{"path": ["wire-cli"], "output": output}]
    }))?;
    let command = context
        .commands
        .first()
        .expect("the fixture context should contain one command");
    command
        .output
        .clone()
        .ok_or_else(|| serde::ser::Error::custom("expected an output contract"))
}

/// A command that declares nothing carries no contract at all.
#[test]
fn absent_output_stays_none() -> Result<(), serde_json::Error> {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "wire-cli.agent_context",
        "package": "wire-cli",
        "commands": [{"path": ["wire-cli"]}]
    }))?;

    assert_eq!(
        context.commands[0].output, None,
        "an undeclared command must not acquire a default contract"
    );
    Ok(())
}

/// An explicit `null` reads as absence, matching the omission rule.
#[test]
fn null_output_reads_as_none() -> Result<(), serde_json::Error> {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "wire-cli.agent_context",
        "package": "wire-cli",
        "commands": [{"path": ["wire-cli"], "output": null}]
    }))?;

    assert_eq!(
        context.commands[0].output, None,
        "null and omission must mean the same thing"
    );
    Ok(())
}

/// An empty block is present but claims nothing.
#[test]
fn empty_output_block_claims_nothing() -> Result<(), serde_json::Error> {
    let output = decode_output(json!({}))?;

    assert_eq!(output, OutputContract::default());
    Ok(())
}

/// Every accepted token decodes to its own variant.
#[rstest]
#[case::data("data", StdoutPurpose::Data)]
#[case::path("path", StdoutPurpose::Path)]
#[case::summary("summary", StdoutPurpose::Summary)]
#[case::none("none", StdoutPurpose::None)]
#[case::unknown("unknown", StdoutPurpose::Unknown)]
fn known_stdout_tokens_decode(
    #[case] token: &str,
    #[case] expected: StdoutPurpose,
) -> Result<(), serde_json::Error> {
    let output = decode_output(json!({"stdout": token}))?;

    assert_eq!(
        output.stdout,
        Some(expected),
        "token {token:?} should decode to {expected:?}"
    );
    Ok(())
}

/// A future token normalizes instead of failing the whole document.
#[test]
fn future_stdout_token_normalizes_to_unknown() -> Result<(), serde_json::Error> {
    let output = decode_output(json!({"stdout": "structured_stream"}))?;

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
    Ok(())
}

/// `unknown` is a stable wire token, so a decoded fallback re-encodes as
/// `unknown` rather than vanishing or reverting to a concrete purpose.
#[test]
fn unknown_serializes_as_the_unknown_token() -> Result<(), serde_json::Error> {
    let contract = OutputContract {
        stdout: Some(StdoutPurpose::Unknown),
        ..OutputContract::default()
    };
    let encoded = serde_json::to_value(&contract)?;

    assert_eq!(
        encoded,
        json!({"stdout": "unknown"}),
        "the fallback needs its own stable spelling"
    );
    Ok(())
}

/// An unrecognized member inside the block is ignored, not fatal.
#[test]
fn future_members_are_tolerated() -> Result<(), serde_json::Error> {
    let output = decode_output(json!({
        "supports_json": true,
        "schema_reference": {"kind": "response", "uri": "schema.json"}
    }))?;

    assert_eq!(
        output.supports_json,
        Some(true),
        "a future member must not discard the declared leaves"
    );
    assert_eq!(
        output.stdout, None,
        "a future member must not invent a stdout purpose"
    );
    Ok(())
}

/// Declared false survives, so refusal and silence stay distinguishable.
#[test]
fn declared_refusal_survives() -> Result<(), serde_json::Error> {
    let output = decode_output(json!({"supports_json": false, "stderr_diagnostics": false}))?;

    assert_eq!(output.supports_json, Some(false));
    assert_eq!(output.stderr_diagnostics, Some(false));
    assert_ne!(
        output.supports_json, None,
        "a refusal is not the same declaration as silence"
    );
    Ok(())
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
fn serialization_omits_absent_members_in_author_order() -> Result<(), serde_json::Error> {
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
    let encoded = serde_json::to_value(&contract)?;

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
    Ok(())
}

/// The round trip preserves every declared member.
#[test]
fn declared_contract_round_trips() -> Result<(), serde_json::Error> {
    let declared = json!({
        "supports_json": true,
        "stdout": "path",
        "stderr_diagnostics": true,
        "exit_classes": [{"name": "success", "meaning": "The command completed"}]
    });
    let output = decode_output(declared.clone())?;

    assert_eq!(
        serde_json::to_value(&output)?,
        declared,
        "a fully declared contract should round trip unchanged"
    );
    Ok(())
}
