//! Tolerant decoding of renderer policy leaves in agent context.
//!
//! ADR-0016 draws the width distinction this module pins: an unrecognized
//! `kind` normalizes to `unknown`, while a known `fixed` kind with a bad column
//! count is a hard error. It also fixes the `null` rule — an optional policy
//! leaf reads as undeclared — and leaves field identifiers opaque, so an empty
//! or repeated identifier is preserved rather than rejected at this boundary.

use std::num::NonZeroU16;

use rstest::rstest;
use serde_json::{Value, json};

use super::*;

/// Decodes the human renderer record of a one-command context.
///
/// Decoding through the document rather than the record exercises the real
/// reader: the role-aware contract decoder supplies the human support default,
/// so each case also proves that a bad or null policy leaf does not disturb it.
fn human_record(record: &Value) -> Result<RendererRecord, serde_json::Error> {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "wire-cli.agent_context",
        "package": "wire-cli",
        "commands": [{"path": ["wire-cli"], "renderer": {"human": record}}]
    }))?;
    let command = context
        .commands
        .first()
        .expect("the fixture context should contain one command");
    Ok(command.renderer.human.clone())
}

/// Builds a non-zero column count for an expected `fixed` width.
fn columns(count: u16) -> NonZeroU16 {
    NonZeroU16::new(count).expect("generated column counts are non-zero")
}

#[rstest]
#[case::terminal(json!({"kind": "terminal"}), RenderWidth::Terminal)]
#[case::unbounded(json!({"kind": "unbounded"}), RenderWidth::Unbounded)]
#[case::fixed_minimum(json!({"kind": "fixed", "columns": 1}), RenderWidth::Fixed { columns: columns(1) })]
#[case::fixed_typical(json!({"kind": "fixed", "columns": 90}), RenderWidth::Fixed { columns: columns(90) })]
#[case::fixed_maximum(json!({"kind": "fixed", "columns": 65535}), RenderWidth::Fixed { columns: columns(65535) })]
fn known_width_objects_decode(#[case] width: Value, #[case] expected: RenderWidth) {
    let record =
        human_record(&json!({"width": width})).expect("a known width object should decode");

    assert_eq!(record.width, Some(expected));
}

#[rstest]
#[case::future_kind(json!({"kind": "future_columns"}))]
#[case::future_kind_with_payload(json!({"kind": "future_columns", "columns": 80, "note": "ignored"}))]
#[case::future_kind_with_bad_payload(json!({"kind": "future_columns", "columns": 0}))]
fn future_width_kinds_normalize_to_unknown(#[case] width: Value) {
    let record = human_record(&json!({"width": width}))
        .expect("an unrecognized kind normalizes instead of failing");

    assert_eq!(record.width, Some(RenderWidth::Unknown));
}

#[rstest]
#[case::missing_columns(json!({"kind": "fixed"}))]
#[case::zero_columns(json!({"kind": "fixed", "columns": 0}))]
#[case::negative_columns(json!({"kind": "fixed", "columns": -1}))]
#[case::fractional_columns(json!({"kind": "fixed", "columns": 1.5}))]
#[case::overflowing_columns(json!({"kind": "fixed", "columns": 65536}))]
#[case::null_columns(json!({"kind": "fixed", "columns": null}))]
#[case::string_columns(json!({"kind": "fixed", "columns": "80"}))]
#[case::missing_kind(json!({"columns": 80}))]
#[case::bare_string(json!("terminal"))]
fn malformed_width_objects_are_rejected(#[case] width: Value) {
    assert!(
        human_record(&json!({"width": width})).is_err(),
        "a known fixed width with a bad column count must fail, not silently become zero"
    );
}

/// Asserts that a future token in any policy family reads as `unknown`.
///
/// The families are listed in one place so a new tolerant enum without the
/// fallback fails here; the macro keeps the per-family body identical.
macro_rules! assert_unknown_token {
    ($($policy:ty),+ $(,)?) => {
        $(
            assert_eq!(
                serde_json::from_value::<$policy>(json!("future_policy_value"))
                    .expect("the tolerant wire enum should accept a future token"),
                <$policy>::Unknown,
                concat!(stringify!($policy), " should normalize a future token to unknown"),
            );
        )+
    };
}

#[test]
fn every_policy_family_normalizes_future_tokens_to_unknown() {
    assert_unknown_token!(
        TtyPolicy,
        ClosedStdinPolicy,
        TogglePolicy,
        AccessibilityPolicy,
        PlainPolicy,
        StreamContent,
        InheritancePolicy,
        RendererFieldLocalization,
    );
}

#[rstest]
#[case::tty(json!({"tty": null}))]
#[case::closed_stdin(json!({"closed_stdin": null}))]
#[case::colour(json!({"colour": null}))]
#[case::emoji(json!({"emoji": null}))]
#[case::progress(json!({"progress": null}))]
#[case::pager(json!({"pager": null}))]
#[case::width(json!({"width": null}))]
#[case::accessibility(json!({"accessibility": null}))]
#[case::plain(json!({"plain": null}))]
#[case::stdout(json!({"stdout": null}))]
#[case::stderr(json!({"stderr": null}))]
#[case::progress_inheritance(json!({"progress_inheritance": null}))]
#[case::subprocess_inheritance(json!({"subprocess_inheritance": null}))]
fn null_policy_leaves_read_as_undeclared(#[case] record: Value) {
    let decoded = human_record(&record).expect("a null policy leaf means undeclared");

    assert_eq!(
        decoded,
        RendererRecord {
            supported: true,
            ..RendererRecord::default()
        },
        "a null leaf must not become a claim, and the role support default still applies"
    );
}

#[rstest]
#[case::empty_identifier(json!([{"id": "", "localization": "localized"}]), vec![""])]
#[case::duplicate_identifiers(
    json!([
        {"id": "error_code", "localization": "localized"},
        {"id": "error_code", "localization": "non_localized"}
    ]),
    vec!["error_code", "error_code"]
)]
fn hand_built_field_identifiers_are_preserved(
    #[case] fields: Value,
    #[case] expected_ids: Vec<&str>,
) {
    let record =
        human_record(&json!({"fields": fields})).expect("identifiers are opaque at this boundary");

    let ids: Vec<&str> = record
        .fields
        .iter()
        .map(|field| field.id.as_str())
        .collect();
    assert_eq!(ids, expected_ids);
}

/// A field with no localization leaf makes no localization claim.
#[test]
fn missing_field_localization_reads_as_unknown() {
    let record = human_record(&json!({"fields": [{"id": "error_code"}]}))
        .expect("a field without localization should decode");

    let [field] = record.fields.as_slice() else {
        panic!("expected one field, got {}", record.fields.len());
    };
    assert_eq!(field.localization, RendererFieldLocalization::Unknown);
}
