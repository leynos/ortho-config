//! Bridge mapping from IR renderer declarations to agent-context records.
//!
//! This is the only place the IR's strict enums meet the wire's tolerant ones.
//! Every variant is mapped explicitly so a dropped arm fails a test rather than
//! silently reporting `unknown`, and nothing here infers, sorts, localizes, or
//! populates `output_modes`.

use ortho_config::{
    AccessibilityPolicy as AgentAccessibilityPolicy, ClosedStdinPolicy as AgentClosedStdinPolicy,
    InheritancePolicy as AgentInheritancePolicy, PlainPolicy as AgentPlainPolicy,
    RenderWidth as AgentRenderWidth, RendererContract, RendererField as AgentRendererField,
    RendererFieldLocalization as AgentFieldLocalization, RendererRecord as AgentRendererRecord,
    StreamContent as AgentStreamContent, TogglePolicy as AgentTogglePolicy,
    TtyPolicy as AgentTtyPolicy,
};

use crate::schema::{
    AccessibilityPolicy, ClosedStdinPolicy, InheritancePolicy, PlainPolicy, RenderWidth,
    RendererField, RendererFieldLocalization, RendererMetadata, RendererRecord, StreamContent,
    TogglePolicy, TtyPolicy,
};

/// Maps an optional IR renderer block onto the agent-context contract.
///
/// An absent block yields the documented legacy support defaults, which
/// [`RendererContract::default`] supplies. A present record with no `supported`
/// leaf keeps the role-specific default rather than becoming `false`.
pub(super) fn map_renderer(declared: Option<&RendererMetadata>) -> RendererContract {
    let mut contract = RendererContract::default();
    let Some(renderer) = declared else {
        return contract;
    };
    if let Some(record) = renderer.human.as_ref() {
        contract.human = map_record(record, contract.human.supported);
    }
    if let Some(record) = renderer.machine.as_ref() {
        contract.machine = map_record(record, contract.machine.supported);
    }
    contract
}

/// Maps one IR renderer record onto its agent-context counterpart.
///
/// Where the IR leaves `supported` undeclared, the role default is preserved by
/// `role_default`, which [`map_renderer`] passes from the contract it is
/// building; here that default is only overridden by an explicit declaration.
fn map_record(record: &RendererRecord, role_default: bool) -> AgentRendererRecord {
    AgentRendererRecord {
        supported: record.supported.unwrap_or(role_default),
        tty: record.tty.map(map_tty),
        closed_stdin: record.closed_stdin.map(map_closed_stdin),
        colour: record.colour.map(map_toggle),
        emoji: record.emoji.map(map_toggle),
        progress: record.progress.map(map_toggle),
        pager: record.pager.map(map_toggle),
        width: record.width.map(map_width),
        accessibility: record.accessibility.map(map_accessibility),
        plain: record.plain.map(map_plain),
        stdout: record.stdout.map(map_stream),
        stderr: record.stderr.map(map_stream),
        progress_inheritance: record.progress_inheritance.map(map_inheritance),
        subprocess_inheritance: record.subprocess_inheritance.map(map_inheritance),
        fields: record.fields.iter().map(map_field).collect(),
    }
}

/// Maps a declared terminal sensitivity.
const fn map_tty(value: TtyPolicy) -> AgentTtyPolicy {
    match value {
        TtyPolicy::Independent => AgentTtyPolicy::Independent,
        TtyPolicy::Adapts => AgentTtyPolicy::Adapts,
        TtyPolicy::Requires => AgentTtyPolicy::Requires,
    }
}

/// Maps a declared closed-stdin policy.
const fn map_closed_stdin(value: ClosedStdinPolicy) -> AgentClosedStdinPolicy {
    match value {
        ClosedStdinPolicy::Ignored => AgentClosedStdinPolicy::Ignored,
        ClosedStdinPolicy::Eof => AgentClosedStdinPolicy::Eof,
        ClosedStdinPolicy::Error => AgentClosedStdinPolicy::Error,
    }
}

/// Maps a declared off/auto/always policy.
const fn map_toggle(value: TogglePolicy) -> AgentTogglePolicy {
    match value {
        TogglePolicy::Never => AgentTogglePolicy::Never,
        TogglePolicy::Auto => AgentTogglePolicy::Auto,
        TogglePolicy::Always => AgentTogglePolicy::Always,
    }
}

/// Maps a declared width, preserving the tagged object shape.
const fn map_width(value: RenderWidth) -> AgentRenderWidth {
    match value {
        RenderWidth::Terminal => AgentRenderWidth::Terminal,
        RenderWidth::Unbounded => AgentRenderWidth::Unbounded,
        RenderWidth::Fixed { columns } => AgentRenderWidth::Fixed { columns },
    }
}

/// Maps a declared accessibility claim.
const fn map_accessibility(value: AccessibilityPolicy) -> AgentAccessibilityPolicy {
    match value {
        AccessibilityPolicy::Unspecified => AgentAccessibilityPolicy::Unspecified,
        AccessibilityPolicy::TextEquivalent => AgentAccessibilityPolicy::TextEquivalent,
    }
}

/// Maps a declared plain-output availability.
const fn map_plain(value: PlainPolicy) -> AgentPlainPolicy {
    match value {
        PlainPolicy::Unsupported => AgentPlainPolicy::Unsupported,
        PlainPolicy::Available => AgentPlainPolicy::Available,
        PlainPolicy::Always => AgentPlainPolicy::Always,
    }
}

/// Maps a declared stream content category.
const fn map_stream(value: StreamContent) -> AgentStreamContent {
    match value {
        StreamContent::None => AgentStreamContent::None,
        StreamContent::Text => AgentStreamContent::Text,
        StreamContent::Structured => AgentStreamContent::Structured,
        StreamContent::Mixed => AgentStreamContent::Mixed,
    }
}

/// Maps a declared inheritance permission.
const fn map_inheritance(value: InheritancePolicy) -> AgentInheritancePolicy {
    match value {
        InheritancePolicy::Allowed => AgentInheritancePolicy::Allowed,
        InheritancePolicy::Forbidden => AgentInheritancePolicy::Forbidden,
    }
}

/// Maps one declared output field, preserving its identifier and order.
fn map_field(field: &RendererField) -> AgentRendererField {
    AgentRendererField {
        id: field.id.clone(),
        localization: map_field_localization(field.localization),
    }
}

/// Maps a declared output-field localization classification.
const fn map_field_localization(value: RendererFieldLocalization) -> AgentFieldLocalization {
    match value {
        RendererFieldLocalization::Localized => AgentFieldLocalization::Localized,
        RendererFieldLocalization::NonLocalized => AgentFieldLocalization::NonLocalized,
    }
}
