//! Renderer-contract property strategies for agent-context round trips.

use super::*;
use proptest::{collection::vec, option, prelude::Just};

/// Builds a renderer contract whose undeclared leaves stay `None`.
///
/// Only `supported` and the declared policy leaves vary, which keeps the
/// strategy small while still exercising every wire-visible field.
pub(super) fn any_agent_renderer() -> impl Strategy<Value = RendererContract> {
    (any_agent_record(), any_agent_record())
        .prop_map(|(human, machine)| RendererContract { human, machine })
}

/// Builds one renderer record with independently chosen leaves.
pub(super) fn any_agent_record() -> impl Strategy<Value = RendererRecord> {
    (
        any::<bool>(),
        option::of(tty_policy()),
        option::of(closed_stdin_policy()),
        option::of(toggle_policy()),
        (
            option::of(render_width()),
            option::of(accessibility_policy()),
            option::of(plain_policy()),
        ),
        (
            option::of(stream_content()),
            option::of(inheritance_policy()),
        ),
        vec(any_agent_renderer_field(), 0..3),
    )
        .prop_map(
            |(
                supported,
                tty,
                closed_stdin,
                colour,
                (width, accessibility, plain),
                (stdout, inheritance),
                fields,
            )| {
                RendererRecord {
                    supported,
                    tty,
                    closed_stdin,
                    colour,
                    emoji: None,
                    progress: None,
                    pager: None,
                    width,
                    accessibility,
                    plain,
                    stdout,
                    stderr: None,
                    progress_inheritance: inheritance,
                    subprocess_inheritance: None,
                    fields,
                }
            },
        )
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
