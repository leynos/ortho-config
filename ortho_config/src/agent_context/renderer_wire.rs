//! Role-aware deserialization for [`RendererContract`].
//!
//! The legacy support default is a property of the slot a record occupies:
//! human records default to supported and machine records to unsupported. A
//! Serde derive cannot express a default that depends on the enclosing field,
//! and a plain `bool` cannot distinguish an absent `supported` leaf from an
//! explicit `false`. This mirror of the wire shape makes both distinctions and
//! then converts into the public types. Everything else, including unknown
//! fields, keeps the ordinary permissive decode.
//!
//! [`RendererContract`]: super::RendererContract

use serde::{Deserialize, Deserializer};

use super::renderer::{
    AccessibilityPolicy, ClosedStdinPolicy, InheritancePolicy, PlainPolicy, RenderWidth,
    RendererContract, RendererField, RendererRecord, StreamContent, TogglePolicy, TtyPolicy,
};

impl<'de> Deserialize<'de> for RendererContract {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ContractWire::deserialize(deserializer)?;
        Ok(Self {
            human: wire.human.into_record(true),
            machine: wire.machine.into_record(false),
        })
    }
}

/// The contract as it appears on the wire, before role defaults are applied.
///
/// Both fields default to an empty record, so a missing record and an empty
/// object take the same path. A present `null` still fails, because a record is
/// not an `Option` here.
#[derive(Deserialize)]
struct ContractWire {
    /// The human slot; defaults to an empty record so the role default applies.
    #[serde(default)]
    human: RecordWire,
    /// The machine slot; defaults to an empty record so the role default applies.
    #[serde(default)]
    machine: RecordWire,
}

/// One renderer record with an explicitly tracked `supported` leaf.
#[derive(Default, Deserialize)]
struct RecordWire {
    /// Declared support, or absence so the enclosing slot's default applies.
    #[serde(default)]
    supported: SupportWire,
    /// Whether the renderer requires a terminal.
    #[serde(default)]
    tty: Option<TtyPolicy>,
    /// What the renderer does when standard input is closed.
    #[serde(default)]
    closed_stdin: Option<ClosedStdinPolicy>,
    /// Whether the renderer emits colour.
    #[serde(default)]
    colour: Option<TogglePolicy>,
    /// Whether the renderer emits emoji.
    #[serde(default)]
    emoji: Option<TogglePolicy>,
    /// Whether the renderer emits progress output.
    #[serde(default)]
    progress: Option<TogglePolicy>,
    /// Whether the renderer pages long output.
    #[serde(default)]
    pager: Option<TogglePolicy>,
    /// The declared render width.
    #[serde(default)]
    width: Option<RenderWidth>,
    /// The declared accessibility policy.
    #[serde(default)]
    accessibility: Option<AccessibilityPolicy>,
    /// Whether plain output is offered.
    #[serde(default)]
    plain: Option<PlainPolicy>,
    /// What the renderer writes to standard output.
    #[serde(default)]
    stdout: Option<StreamContent>,
    /// What the renderer writes to standard error.
    #[serde(default)]
    stderr: Option<StreamContent>,
    /// Whether child processes inherit the progress stream.
    #[serde(default)]
    progress_inheritance: Option<InheritancePolicy>,
    /// Whether child processes inherit the renderer's streams.
    #[serde(default)]
    subprocess_inheritance: Option<InheritancePolicy>,
    /// Per-field localization claims.
    #[serde(default)]
    fields: Vec<RendererField>,
}

impl RecordWire {
    /// Converts to the public record, substituting the slot's legacy default
    /// when `supported` was not declared.
    fn into_record(self, default_supported: bool) -> RendererRecord {
        RendererRecord {
            supported: self.supported.or_default(default_supported),
            tty: self.tty,
            closed_stdin: self.closed_stdin,
            colour: self.colour,
            emoji: self.emoji,
            progress: self.progress,
            pager: self.pager,
            width: self.width,
            accessibility: self.accessibility,
            plain: self.plain,
            stdout: self.stdout,
            stderr: self.stderr,
            progress_inheritance: self.progress_inheritance,
            subprocess_inheritance: self.subprocess_inheritance,
            fields: self.fields,
        }
    }
}

/// Whether the `supported` leaf was declared, and its value when it was.
#[derive(Default)]
enum SupportWire {
    /// The leaf was absent, so the slot's legacy default applies.
    #[default]
    Absent,
    /// The leaf was declared explicitly.
    Declared(bool),
}

impl SupportWire {
    /// Resolves the declared value, falling back to the slot default.
    const fn or_default(self, default_supported: bool) -> bool {
        match self {
            Self::Absent => default_supported,
            Self::Declared(supported) => supported,
        }
    }
}

impl<'de> Deserialize<'de> for SupportWire {
    /// Reads a declared boolean. A `null` leaf fails rather than reading as
    /// absence, because the documented contract rejects a present null.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        bool::deserialize(deserializer).map(Self::Declared)
    }
}
