//! Golden snapshots for the structured-output declaration (E4).
//!
//! The tool re-exports the library IR's types, so these snapshots pin the wire
//! spelling the tool actually emits: a member added or renamed, or a
//! `skip_serializing_if` removed, shows up as a diff instead of passing
//! silently.
//!
//! Undeclared members are absent from these snapshots rather than `null`,
//! because that omission is what lets a consumer tell an author's refusal from
//! an author's silence.

use insta::assert_snapshot;

use super::super::{ExitClass, OutputMetadata, StdoutPurpose};

/// Renders one declaration as canonical pretty JSON for snapshotting.
fn snapshot_json(metadata: &OutputMetadata) -> String {
    serde_json::to_string_pretty(metadata).expect("output metadata serialises to JSON")
}

/// An empty declaration is present but claims nothing.
#[test]
fn empty_output_serializes_as_an_empty_object() {
    assert_snapshot!(snapshot_json(&OutputMetadata::default()));
}

/// A fully declared block writes every member in declaration order.
#[test]
fn fully_declared_output_serializes_every_member() {
    assert_snapshot!(snapshot_json(&OutputMetadata {
        supports_json: Some(true),
        stdout: Some(StdoutPurpose::Data),
        stderr_diagnostics: Some(false),
        exit_classes: vec![
            ExitClass {
                name: "success".to_owned(),
                meaning: "The command completed".to_owned(),
            },
            ExitClass {
                name: "usage".to_owned(),
                meaning: "The invocation was invalid".to_owned(),
            },
        ],
    }));
}

/// A declared refusal keeps its `false` rather than collapsing to omission.
#[test]
fn refused_json_serializes_as_false() {
    assert_snapshot!(snapshot_json(&OutputMetadata {
        supports_json: Some(false),
        stdout: Some(StdoutPurpose::None),
        stderr_diagnostics: None,
        exit_classes: Vec::new(),
    }));
}

/// Each stdout purpose has its own wire token.
#[test]
fn stdout_purposes_serialize_their_tokens() {
    let tokens = [
        StdoutPurpose::Data,
        StdoutPurpose::Path,
        StdoutPurpose::Summary,
        StdoutPurpose::None,
    ]
    .map(|purpose| {
        serde_json::to_value(OutputMetadata {
            stdout: Some(purpose),
            ..OutputMetadata::default()
        })
        .expect("output metadata serialises to JSON")
    });

    assert_snapshot!(serde_json::to_string_pretty(&tokens).expect("tokens serialise"));
}
