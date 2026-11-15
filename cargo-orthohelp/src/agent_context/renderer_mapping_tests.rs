//! Enum-by-enum bridge mapping tests for renderer metadata (E1).
//!
//! Each accepted IR variant has an explicit case, so a dropped or swapped
//! `match` arm fails here instead of silently degrading to `unknown`.

use super::renderer_tests::bridge_single;
use ortho_config::{
    AccessibilityPolicy, ClosedStdinPolicy, InheritancePolicy, PlainPolicy, RenderWidth,
    RendererField, RendererFieldLocalization, RendererRecord, StreamContent, TogglePolicy,
    TtyPolicy,
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
fn human_only(record: IrRecord) -> RendererMetadata {
    RendererMetadata {
        human: Some(record),
        machine: None,
    }
}

/// Bridges a human record declaring one policy leaf and returns the human slot.
///
/// The single-leaf policy families below all perform this step, so it lives in
/// one place rather than being repeated in each mapping test.
fn bridged_human(record: IrRecord) -> RendererRecord {
    bridge_single(Some(human_only(record))).human
}

/// Declares a variant-for-variant mapping test for a single-leaf policy.
///
/// The closed-stdin, width, accessibility, and plain-output families differ
/// only in their IR enum, wire enum, and record field, so the test shape lives
/// in one macro rather than four near-identical functions.
macro_rules! single_leaf_mapping_test {
    (
        $test:ident,
        $ir_type:ty,
        $wire_type:ty,
        $leaf:ident;
        $($case_name:ident: $ir_variant:ident => $wire_variant:ident),+ $(,)?
    ) => {
        #[doc = concat!(
            "Every `", stringify!($ir_type),
            "` variant maps to its exact wire counterpart."
        )]
        #[rstest]
        $(
            #[case::$case_name($ir_type::$ir_variant, $wire_type::$wire_variant)]
        )+
        fn $test(#[case] ir: $ir_type, #[case] expected: $wire_type) {
            let human = bridged_human(IrRecord {
                $leaf: Some(ir),
                ..IrRecord::default()
            });

            assert_eq!(human.$leaf, Some(expected));
        }
    };
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
    let renderer = bridge_single(Some(human_only(IrRecord {
        tty: Some(ir),
        ..IrRecord::default()
    })));

    assert_eq!(renderer.human.tty, Some(expected));
    assert_eq!(
        renderer.machine.tty, None,
        "the machine record is untouched"
    );
}

single_leaf_mapping_test!(
    closed_stdin_policy_maps_exactly,
    IrClosedStdin,
    ClosedStdinPolicy,
    closed_stdin;
    ignored: Ignored => Ignored,
    eof: Eof => Eof,
    error: Error => Error,
);

/// One of the four toggle leaves on a renderer record.
///
/// The four leaves share a three-variant vocabulary, so declaring them all with
/// the same value — as a single case can only do — would hide a bridge that
/// copies one leaf's value into another's slot. Naming the leaves lets the test
/// declare one at a time and check where the value actually lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToggleLeaf {
    Colour,
    Emoji,
    Progress,
    Pager,
}

impl ToggleLeaf {
    /// Every toggle leaf, in declaration order.
    const ALL: [Self; 4] = [Self::Colour, Self::Emoji, Self::Progress, Self::Pager];

    /// Builds a record that declares only this leaf.
    fn declare(self, ir: IrToggle) -> IrRecord {
        let mut record = IrRecord::default();
        match self {
            Self::Colour => record.colour = Some(ir),
            Self::Emoji => record.emoji = Some(ir),
            Self::Progress => record.progress = Some(ir),
            Self::Pager => record.pager = Some(ir),
        }
        record
    }

    /// Reads this leaf back from a bridged record.
    fn read(self, human: &RendererRecord) -> Option<TogglePolicy> {
        match self {
            Self::Colour => human.colour,
            Self::Emoji => human.emoji,
            Self::Progress => human.progress,
            Self::Pager => human.pager,
        }
    }
}

/// Every IR toggle variant maps to the exact leaf it was declared on.
///
/// Each case declares one leaf at a time, so a bridge that assigned a toggle to
/// the wrong field leaves the declared leaf empty and fills a sibling. Both
/// halves of that mistake fail here; declaring all four at once would give them
/// equal values and let the swap pass.
#[rstest]
#[case::never(IrToggle::Never, TogglePolicy::Never)]
#[case::auto(IrToggle::Auto, TogglePolicy::Auto)]
#[case::always(IrToggle::Always, TogglePolicy::Always)]
fn toggle_policy_maps_exactly(#[case] ir: IrToggle, #[case] expected: TogglePolicy) {
    for declared in ToggleLeaf::ALL {
        let human = bridged_human(declared.declare(ir));
        for leaf in ToggleLeaf::ALL {
            let want = (leaf == declared).then_some(expected);
            assert_eq!(
                leaf.read(&human),
                want,
                "{leaf:?} after declaring only {declared:?}"
            );
        }
    }
}

single_leaf_mapping_test!(
    width_shape_maps_exactly,
    IrWidth,
    RenderWidth,
    width;
    terminal: Terminal => Terminal,
    unbounded: Unbounded => Unbounded,
);

/// A declared fixed width keeps its exact column count.
#[test]
fn fixed_width_preserves_column_count() {
    let columns = std::num::NonZeroU16::new(80).expect("80 is non-zero");
    let renderer = bridge_single(Some(human_only(IrRecord {
        width: Some(IrWidth::Fixed { columns }),
        ..IrRecord::default()
    })));

    assert_eq!(renderer.human.width, Some(RenderWidth::Fixed { columns }));
}

single_leaf_mapping_test!(
    accessibility_policy_maps_exactly,
    IrAccessibility,
    AccessibilityPolicy,
    accessibility;
    unspecified: Unspecified => Unspecified,
    text_equivalent: TextEquivalent => TextEquivalent,
);

single_leaf_mapping_test!(
    plain_policy_maps_exactly,
    IrPlain,
    PlainPolicy,
    plain;
    unsupported: Unsupported => Unsupported,
    available: Available => Available,
    always: Always => Always,
);

/// Every IR stream-content variant maps to its exact wire counterpart.
#[rstest]
#[case::none(IrStream::None, StreamContent::None)]
#[case::text(IrStream::Text, StreamContent::Text)]
#[case::structured(IrStream::Structured, StreamContent::Structured)]
#[case::mixed(IrStream::Mixed, StreamContent::Mixed)]
fn stream_content_maps_exactly(#[case] ir: IrStream, #[case] expected: StreamContent) {
    let renderer = bridge_single(Some(human_only(IrRecord {
        stdout: Some(ir),
        stderr: Some(ir),
        ..IrRecord::default()
    })));

    assert_eq!(renderer.human.stdout, Some(expected));
    assert_eq!(renderer.human.stderr, Some(expected));
}

/// Every IR inheritance variant maps to its exact wire counterpart.
#[rstest]
#[case::allowed(IrInheritance::Allowed, InheritancePolicy::Allowed)]
#[case::forbidden(IrInheritance::Forbidden, InheritancePolicy::Forbidden)]
fn inheritance_policy_maps_exactly(#[case] ir: IrInheritance, #[case] expected: InheritancePolicy) {
    let renderer = bridge_single(Some(human_only(IrRecord {
        progress_inheritance: Some(ir),
        subprocess_inheritance: Some(ir),
        ..IrRecord::default()
    })));

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
    let renderer = bridge_single(Some(human_only(IrRecord {
        fields: vec![IrField {
            id: "error_code".to_owned(),
            localization: ir,
        }],
        ..IrRecord::default()
    })));

    assert_eq!(
        renderer.human.fields,
        [RendererField {
            id: "error_code".to_owned(),
            localization: expected,
        }]
    );
}
