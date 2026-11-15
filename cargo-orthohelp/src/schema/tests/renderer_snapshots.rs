//! Golden snapshots for the tool's renderer mirror (E2).
//!
//! The mirror repeats the library IR's shape rather than sharing its types, so
//! these snapshots are what pin the two spellings together. A field added to one
//! side without the other changes a snapshot, and a wire token renamed anywhere
//! shows up as a diff rather than passing silently.

use insta::assert_snapshot;

use super::super::{
    PlainPolicy, RenderWidth, RendererField, RendererFieldLocalization, RendererMetadata,
    RendererRecord, StreamContent, TtyPolicy,
};

/// Renders one declaration as canonical pretty JSON for snapshotting.
fn snapshot_json(metadata: &RendererMetadata) -> String {
    serde_json::to_string_pretty(metadata).expect("renderer metadata serialises to JSON")
}

/// Builds a record declaring one leaf of every kind.
fn fully_declared_record() -> RendererRecord {
    RendererRecord {
        supported: Some(true),
        tty: Some(TtyPolicy::Adapts),
        colour: Some(super::super::TogglePolicy::Auto),
        width: Some(RenderWidth::Fixed {
            columns: std::num::NonZeroU16::new(80).expect("80 is non-zero"),
        }),
        plain: Some(PlainPolicy::Available),
        stdout: Some(StreamContent::Text),
        fields: vec![RendererField {
            id: "error_code".to_owned(),
            localization: RendererFieldLocalization::NonLocalized,
        }],
        ..RendererRecord::default()
    }
}

/// An absent block stays a JSON null so consumers see "undeclared".
#[test]
fn absent_renderer_serializes_as_null() {
    assert_snapshot!(snapshot_json(&RendererMetadata::default()));
}

/// A human-only declaration leaves the machine record null.
#[test]
fn human_only_renderer_serializes_its_record_and_null() {
    assert_snapshot!(snapshot_json(&RendererMetadata {
        human: Some(fully_declared_record()),
        machine: None,
    }));
}

/// A machine-only declaration leaves the human record null.
#[test]
fn machine_only_renderer_serializes_its_record_and_null() {
    assert_snapshot!(snapshot_json(&RendererMetadata {
        human: None,
        machine: Some(fully_declared_record()),
    }));
}

/// Both records are kept independent; neither inherits the other's leaves.
#[test]
fn dual_renderer_serializes_both_records_independently() {
    assert_snapshot!(snapshot_json(&RendererMetadata {
        human: Some(fully_declared_record()),
        machine: Some(RendererRecord {
            supported: Some(false),
            stdout: Some(StreamContent::Structured),
            ..RendererRecord::default()
        }),
    }));
}
