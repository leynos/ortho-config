//! Role-specific support defaults for the agent renderer contract.
//!
//! ADR-0016 makes the legacy support default a property of the renderer slot,
//! not of the record: a human record defaults to supported and a machine
//! record to unsupported. These cases pin that table at the reader boundary,
//! including the distinction between an absent `supported` leaf and an
//! explicit `false`.

use super::*;
use rstest::rstest;
use serde_json::{Value, json};

/// Builds a one-command context whose command carries `renderer`.
fn context_with_renderer(renderer: &Value) -> Result<AgentContext, serde_json::Error> {
    serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "defaults-cli.agent_context",
        "package": "defaults-cli",
        "commands": [{"path": ["defaults-cli"], "renderer": renderer}]
    }))
}

/// Reads the declared support pair from a context built by
/// [`context_with_renderer`].
fn support_pair(context: &AgentContext) -> (bool, bool) {
    let command = context
        .commands
        .first()
        .expect("the fixture context should contain one command");
    (
        command.renderer.human.supported,
        command.renderer.machine.supported,
    )
}

#[test]
fn missing_renderer_applies_role_specific_support_defaults() {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "defaults-cli.agent_context",
        "package": "defaults-cli",
        "commands": [{"path": ["defaults-cli"]}]
    }))
    .expect("deserialize a command without a renderer field");

    assert_eq!(support_pair(&context), (true, false));
}

#[rstest]
#[case::empty_renderer(json!({}), (true, false))]
#[case::empty_records(json!({"human": {}, "machine": {}}), (true, false))]
#[case::missing_human_record(json!({"machine": {}}), (true, false))]
#[case::missing_machine_record(json!({"human": {}}), (true, false))]
#[case::explicit_human_false(json!({"human": {"supported": false}}), (false, false))]
#[case::explicit_machine_true(json!({"machine": {"supported": true}}), (true, true))]
#[case::both_explicit(json!({
    "human": {"supported": false},
    "machine": {"supported": true}
}), (false, true))]
fn empty_and_partial_renderers_apply_role_specific_defaults(
    #[case] renderer: Value,
    #[case] expected: (bool, bool),
) {
    let context =
        context_with_renderer(&renderer).expect("an empty or partial renderer is well-formed");

    assert_eq!(support_pair(&context), expected);
}

#[rstest]
#[case::null_renderer(json!(null))]
#[case::null_human_record(json!({"human": null}))]
#[case::null_machine_record(json!({"machine": null}))]
#[case::null_supported(json!({"human": {"supported": null}}))]
#[case::null_fields(json!({"human": {"fields": null}}))]
fn null_renderer_parts_are_rejected(#[case] renderer: Value) {
    assert!(
        context_with_renderer(&renderer).is_err(),
        "a present null is invalid in agent context"
    );
}
