//! Renderer-contract property strategies for agent-context round trips.

use super::*;
use proptest::{collection::vec, option, prelude::Just};

/// Builds a renderer contract from two independently generated records.
///
/// Each role is generated separately, so a case can declare every policy leaf
/// on one side and none on the other.
pub(super) fn any_agent_renderer() -> impl Strategy<Value = RendererContract> {
    (any_agent_record(), any_agent_record())
        .prop_map(|(human, machine)| RendererContract { human, machine })
}

/// Number of toggle leaves in one renderer record.
const TOGGLE_LEAVES: usize = 4;
/// Number of stream-category leaves in one renderer record.
const STREAM_LEAVES: usize = 2;
/// Number of inheritance leaves in one renderer record.
const INHERITANCE_LEAVES: usize = 2;

/// Builds one renderer record with independently chosen leaves.
///
/// Every optional leaf is drawn on its own, including the four toggles, both
/// stream categories, and both inheritance claims: the wire record is copied
/// field by field, so a coupled pair would hide a swapped assignment.
///
/// The repeated families are generated as fixed-length vectors rather than
/// nested tuples. Both draw the leaves independently; the flat form keeps the
/// strategy's debug-build stack use below the 2 MiB test-thread default, which
/// a deeper tuple nest overruns.
pub(super) fn any_agent_record() -> impl Strategy<Value = RendererRecord> {
    (
        any::<bool>(),
        option::of(tty_policy()),
        option::of(closed_stdin_policy()),
        vec(option::of(toggle_policy()), TOGGLE_LEAVES),
        option::of(render_width()),
        option::of(accessibility_policy()),
        option::of(plain_policy()),
        vec(option::of(stream_content()), STREAM_LEAVES),
        vec(option::of(inheritance_policy()), INHERITANCE_LEAVES),
        vec(any_agent_renderer_field(), 0..3),
    )
        .prop_map(
            |(
                supported,
                tty,
                closed_stdin,
                toggles,
                width,
                accessibility,
                plain,
                streams,
                inheritances,
                fields,
            )| {
                let [colour, emoji, progress, pager] = exact_leaves(toggles);
                let [stdout, stderr] = exact_leaves(streams);
                let [progress_inheritance, subprocess_inheritance] = exact_leaves(inheritances);
                RendererRecord {
                    supported,
                    tty,
                    closed_stdin,
                    colour,
                    emoji,
                    progress,
                    pager,
                    width,
                    accessibility,
                    plain,
                    stdout,
                    stderr,
                    progress_inheritance,
                    subprocess_inheritance,
                    fields,
                }
            },
        )
}

/// Converts a generated fixed-length family into its named leaves.
///
/// The strategy fixes the length, so a mismatch is a generator defect rather
/// than an input case.
fn exact_leaves<T, const N: usize>(leaves: Vec<T>) -> [T; N] {
    let Ok(exact) = leaves.try_into() else {
        panic!("the generating strategy must produce exactly {N} leaves");
    };
    exact
}

/// Builds one declared output field.
pub(super) fn any_agent_renderer_field() -> impl Strategy<Value = RendererField> {
    (summary(), field_localization())
        .prop_map(|(id, localization)| RendererField { id, localization })
}

/// Declared terminal-sensitivity tokens, including the tolerant fallback.
pub(super) fn tty_policy() -> impl Strategy<Value = TtyPolicy> {
    prop_oneof![
        Just(TtyPolicy::Independent),
        Just(TtyPolicy::Adapts),
        Just(TtyPolicy::Requires),
        Just(TtyPolicy::Unknown),
    ]
}

/// Declared closed-stdin tokens, including the tolerant fallback.
pub(super) fn closed_stdin_policy() -> impl Strategy<Value = ClosedStdinPolicy> {
    prop_oneof![
        Just(ClosedStdinPolicy::Ignored),
        Just(ClosedStdinPolicy::Eof),
        Just(ClosedStdinPolicy::Error),
        Just(ClosedStdinPolicy::Unknown),
    ]
}

/// Declared off/auto/always tokens, including the tolerant fallback.
pub(super) fn toggle_policy() -> impl Strategy<Value = TogglePolicy> {
    prop_oneof![
        Just(TogglePolicy::Never),
        Just(TogglePolicy::Auto),
        Just(TogglePolicy::Always),
        Just(TogglePolicy::Unknown),
    ]
}

/// Declared width shapes, including a bounded fixed width.
pub(super) fn render_width() -> impl Strategy<Value = RenderWidth> {
    prop_oneof![
        Just(RenderWidth::Terminal),
        Just(RenderWidth::Unbounded),
        (1u16..=200).prop_map(|columns| RenderWidth::Fixed {
            columns: std::num::NonZeroU16::new(columns).expect("range excludes zero"),
        }),
        Just(RenderWidth::Unknown),
    ]
}

/// Declared accessibility claims, including the tolerant fallback.
pub(super) fn accessibility_policy() -> impl Strategy<Value = AccessibilityPolicy> {
    prop_oneof![
        Just(AccessibilityPolicy::Unspecified),
        Just(AccessibilityPolicy::TextEquivalent),
        Just(AccessibilityPolicy::Unknown),
    ]
}

/// Declared plain-output availability, including the tolerant fallback.
pub(super) fn plain_policy() -> impl Strategy<Value = PlainPolicy> {
    prop_oneof![
        Just(PlainPolicy::Unsupported),
        Just(PlainPolicy::Available),
        Just(PlainPolicy::Always),
        Just(PlainPolicy::Unknown),
    ]
}

/// Declared stream content categories, including the tolerant fallback.
pub(super) fn stream_content() -> impl Strategy<Value = StreamContent> {
    prop_oneof![
        Just(StreamContent::None),
        Just(StreamContent::Text),
        Just(StreamContent::Structured),
        Just(StreamContent::Mixed),
        Just(StreamContent::Unknown),
    ]
}

/// Declared inheritance permissions, including the tolerant fallback.
pub(super) fn inheritance_policy() -> impl Strategy<Value = InheritancePolicy> {
    prop_oneof![
        Just(InheritancePolicy::Allowed),
        Just(InheritancePolicy::Forbidden),
        Just(InheritancePolicy::Unknown),
    ]
}

/// Declared field localization classifications, including the fallback.
pub(super) fn field_localization() -> impl Strategy<Value = RendererFieldLocalization> {
    prop_oneof![
        Just(RendererFieldLocalization::Localized),
        Just(RendererFieldLocalization::NonLocalized),
        Just(RendererFieldLocalization::Unknown),
    ]
}
