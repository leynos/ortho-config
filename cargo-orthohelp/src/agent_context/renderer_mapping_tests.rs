//! Enum-by-enum bridge mapping tests for renderer metadata (E1).
//!
//! Each accepted IR variant has an explicit case, so a dropped or swapped
//! `match` arm fails here instead of silently degrading to `unknown`.

use super::renderer_tests::bridge_single;
use ortho_config::{
    AccessibilityPolicy, ClosedStdinPolicy, InheritancePolicy, PlainPolicy, RenderWidth,
    RendererField, RendererFieldLocalization, StreamContent, TogglePolicy, TtyPolicy,
};
use rstest::rstest;

use crate::schema::{
    AccessibilityPolicy as IrAccessibility, ClosedStdinPolicy as IrClosedStdin,
    InheritancePolicy as IrInheritance, PlainPolicy as IrPlain, RenderWidth as IrWidth,
    RendererField as IrField, RendererFieldLocalization as IrFieldLocalization, RendererMetadata,
    RendererRecord as IrRecord, StreamContent as IrStream, TogglePolicy as IrToggle,
    TtyPolicy as IrTty,
};

/// Builds metadata whose human record carries `record` and no machine record.
///
/// The machine record stays absent so every case also proves the bridge leaves
/// the untouched slot empty rather than copying the declared one across.
fn human_only(record: IrRecord) -> Option<RendererMetadata> {
    Some(RendererMetadata {
        human: Some(record),
        machine: None,
    })
}

/// Every IR terminal-sensitivity variant maps to its exact wire counterpart.
///
/// A swapped human/machine mapping fails here because the two records are
/// declared with different tokens.
#[rstest]
#[case::independent(IrTty::Independent, TtyPolicy::Independent)]
#[case::adapts(IrTty::Adapts, TtyPolicy::Adapts)]
#[case::requires(IrTty::Requires, TtyPolicy::Requires)]
fn tty_policy_maps_exactly(#[case] ir: IrTty, #[case] expected: TtyPolicy) {
    let renderer = bridge_single(human_only(IrRecord {
        tty: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.tty, Some(expected));
    assert_eq!(
        renderer.machine.tty, None,
        "the machine record is untouched"
    );
}

/// Every IR closed-stdin variant maps to its exact wire counterpart.
#[rstest]
#[case::ignored(IrClosedStdin::Ignored, ClosedStdinPolicy::Ignored)]
#[case::eof(IrClosedStdin::Eof, ClosedStdinPolicy::Eof)]
#[case::error(IrClosedStdin::Error, ClosedStdinPolicy::Error)]
fn closed_stdin_policy_maps_exactly(
    #[case] ir: IrClosedStdin,
    #[case] expected: ClosedStdinPolicy,
) {
    let renderer = bridge_single(human_only(IrRecord {
        closed_stdin: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.closed_stdin, Some(expected));
}

/// Every IR toggle variant maps to its exact wire counterpart.
#[rstest]
#[case::never(IrToggle::Never, TogglePolicy::Never)]
#[case::auto(IrToggle::Auto, TogglePolicy::Auto)]
#[case::always(IrToggle::Always, TogglePolicy::Always)]
fn toggle_policy_maps_exactly(#[case] ir: IrToggle, #[case] expected: TogglePolicy) {
    let renderer = bridge_single(human_only(IrRecord {
        colour: Some(ir),
        emoji: Some(ir),
        progress: Some(ir),
        pager: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.colour, Some(expected));
    assert_eq!(renderer.human.emoji, Some(expected));
    assert_eq!(renderer.human.progress, Some(expected));
    assert_eq!(renderer.human.pager, Some(expected));
}

/// Every IR width shape maps to its exact wire counterpart, including `fixed`.
#[rstest]
#[case::terminal(IrWidth::Terminal, RenderWidth::Terminal)]
#[case::unbounded(IrWidth::Unbounded, RenderWidth::Unbounded)]
fn width_shape_maps_exactly(#[case] ir: IrWidth, #[case] expected: RenderWidth) {
    let renderer = bridge_single(human_only(IrRecord {
        width: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.width, Some(expected));
}

/// A declared fixed width keeps its exact column count.
#[test]
fn fixed_width_preserves_column_count() {
    let columns = std::num::NonZeroU16::new(80).expect("80 is non-zero");
    let renderer = bridge_single(human_only(IrRecord {
        width: Some(IrWidth::Fixed { columns }),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.width, Some(RenderWidth::Fixed { columns }));
}

/// Every IR accessibility variant maps to its exact wire counterpart.
#[rstest]
#[case::unspecified(IrAccessibility::Unspecified, AccessibilityPolicy::Unspecified)]
#[case::text_equivalent(IrAccessibility::TextEquivalent, AccessibilityPolicy::TextEquivalent)]
fn accessibility_policy_maps_exactly(
    #[case] ir: IrAccessibility,
    #[case] expected: AccessibilityPolicy,
) {
    let renderer = bridge_single(human_only(IrRecord {
        accessibility: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.accessibility, Some(expected));
}

/// Every IR plain-output variant maps to its exact wire counterpart.
#[rstest]
#[case::unsupported(IrPlain::Unsupported, PlainPolicy::Unsupported)]
#[case::available(IrPlain::Available, PlainPolicy::Available)]
#[case::always(IrPlain::Always, PlainPolicy::Always)]
fn plain_policy_maps_exactly(#[case] ir: IrPlain, #[case] expected: PlainPolicy) {
    let renderer = bridge_single(human_only(IrRecord {
        plain: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.plain, Some(expected));
}

/// Every IR stream-content variant maps to its exact wire counterpart.
#[rstest]
#[case::none(IrStream::None, StreamContent::None)]
#[case::text(IrStream::Text, StreamContent::Text)]
#[case::structured(IrStream::Structured, StreamContent::Structured)]
#[case::mixed(IrStream::Mixed, StreamContent::Mixed)]
fn stream_content_maps_exactly(#[case] ir: IrStream, #[case] expected: StreamContent) {
    let renderer = bridge_single(human_only(IrRecord {
        stdout: Some(ir),
        stderr: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.stdout, Some(expected));
    assert_eq!(renderer.human.stderr, Some(expected));
}

/// Every IR inheritance variant maps to its exact wire counterpart.
#[rstest]
#[case::allowed(IrInheritance::Allowed, InheritancePolicy::Allowed)]
#[case::forbidden(IrInheritance::Forbidden, InheritancePolicy::Forbidden)]
fn inheritance_policy_maps_exactly(#[case] ir: IrInheritance, #[case] expected: InheritancePolicy) {
    let renderer = bridge_single(human_only(IrRecord {
        progress_inheritance: Some(ir),
        subprocess_inheritance: Some(ir),
        ..IrRecord::default()
    }));

    assert_eq!(renderer.human.progress_inheritance, Some(expected));
    assert_eq!(renderer.human.subprocess_inheritance, Some(expected));
}

/// Every IR field-localization variant maps to its exact wire counterpart.
#[rstest]
#[case::localized(IrFieldLocalization::Localized, RendererFieldLocalization::Localized)]
#[case::non_localized(
    IrFieldLocalization::NonLocalized,
    RendererFieldLocalization::NonLocalized
)]
fn field_localization_maps_exactly(
    #[case] ir: IrFieldLocalization,
    #[case] expected: RendererFieldLocalization,
) {
    let renderer = bridge_single(human_only(IrRecord {
        fields: vec![IrField {
            id: "error_code".to_owned(),
            localization: ir,
        }],
        ..IrRecord::default()
    }));

    assert_eq!(
        renderer.human.fields,
        [RendererField {
            id: "error_code".to_owned(),
            localization: expected,
        }]
    );
}
