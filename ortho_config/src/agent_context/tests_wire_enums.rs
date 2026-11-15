//! Canonical wire spellings for agent-context enums.

use super::*;
use rstest::rstest;

#[rstest]
#[case(AsyncSubmissionMode::Inline, "inline")]
#[case(AsyncSubmissionMode::Submit, "submit")]
fn async_submission_mode_serializes_canonical_wire_values(
    #[case] mode: AsyncSubmissionMode,
    #[case] expected: &str,
) {
    let value = serde_json::to_value(mode).expect("serialize async submission mode");
    assert_eq!(value, expected);
}

#[rstest]
#[case(InteractionMode::Unknown, "unknown")]
#[case(InteractionMode::NonInteractive, "non_interactive")]
#[case(InteractionMode::Interactive, "interactive")]
fn interaction_mode_serializes_canonical_wire_values(
    #[case] mode: InteractionMode,
    #[case] expected: &str,
) {
    let value = serde_json::to_value(mode).expect("serialize interaction mode");
    assert_eq!(value, expected);
}

#[rstest]
#[case(PolicyMode::Off, "off")]
#[case(PolicyMode::Warn, "warn")]
#[case(PolicyMode::Deny, "deny")]
fn policy_mode_serializes_canonical_wire_values(#[case] mode: PolicyMode, #[case] expected: &str) {
    let value = serde_json::to_value(mode).expect("serialize policy mode");
    assert_eq!(value, expected);
}

#[rstest]
#[case(MutationEffect::Unknown, "unknown")]
#[case(MutationEffect::ReadOnly, "read_only")]
#[case(MutationEffect::Write, "write")]
#[case(MutationEffect::Delete, "delete")]
#[case(MutationEffect::Submit, "submit")]
fn mutation_effect_serializes_canonical_wire_values(
    #[case] effect: MutationEffect,
    #[case] expected: &str,
) {
    let value = serde_json::to_value(effect).expect("serialize mutation effect");
    assert_eq!(value, expected);
}
