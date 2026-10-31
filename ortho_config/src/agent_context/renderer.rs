//! Renderer contracts for agent context (roadmap 7.2.2).
//!
//! Split from `mod` so each module stays beneath the 400-line cap. These types
//! are the tolerant reading side of the renderer metadata: unlike the
//! documentation IR, every policy enum carries an `Unknown` fallback so a
//! consumer that meets a future token keeps reading the rest of the document.
//!
//! See ADR-0016 for the vocabulary and the role-specific legacy support
//! defaults.

use std::num::NonZeroU16;

use serde::{Deserialize, Deserializer, Serialize};

/// Declared rendering for one command in agent context.
///
/// Serialization of the all-default value is exactly
/// `{"human":{"supported":true},"machine":{"supported":false}}`: the
/// compatibility defaults are explicit, while undeclared policy stays absent
/// rather than becoming an affirmative promise.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RendererContract {
    /// Human-facing rendering description.
    pub human: RendererRecord,
    /// Machine-facing rendering description.
    pub machine: RendererRecord,
}

impl Default for RendererContract {
    fn default() -> Self {
        Self {
            human: default_human_record(),
            machine: default_machine_record(),
        }
    }
}

/// Supplied legacy human support default.
///
/// `true` is a compatibility value, not an observed runtime capability, and it
/// is deliberately not [`SupportDeclaration::default`]'s `false`.
///
/// [`SupportDeclaration::default`]: crate::SupportDeclaration
fn default_human_record() -> RendererRecord {
    RendererRecord {
        supported: true,
        ..RendererRecord::default()
    }
}

/// Supplied legacy machine support default.
///
/// `false` matches the pre-renderer schema, where no command declared machine
/// output.
fn default_machine_record() -> RendererRecord {
    RendererRecord::default()
}

/// One renderer's declared presentation in agent context.
///
/// `supported` defaults to `false` so a record read without the leaf is still
/// well-formed; the role-specific default is applied by
/// [`RendererContract`]'s field attributes. Undeclared policy fields are
/// omitted from the wire.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RendererRecord {
    /// Whether this renderer is supported.
    #[serde(default)]
    pub supported: bool,
    /// Terminal (TTY) sensitivity of stdout; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty: Option<TtyPolicy>,
    /// Behaviour when stdin is closed; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_stdin: Option<ClosedStdinPolicy>,
    /// Colour policy; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<TogglePolicy>,
    /// Emoji policy; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<TogglePolicy>,
    /// Progress-animation policy; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<TogglePolicy>,
    /// Pager policy; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pager: Option<TogglePolicy>,
    /// Declared output width; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<RenderWidth>,
    /// Accessibility claim; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<AccessibilityPolicy>,
    /// Plain-output availability; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plain: Option<PlainPolicy>,
    /// Declared stdout content category; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<StreamContent>,
    /// Declared stderr content category; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<StreamContent>,
    /// Whether progress output may pass through; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_inheritance: Option<InheritancePolicy>,
    /// Whether subprocess output may pass through; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subprocess_inheritance: Option<InheritancePolicy>,
    /// Output-field localization claims in declaration order.
    ///
    /// Omitted when empty, which makes no claim about undeclared fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<RendererField>,
}

/// One output field and whether its value may be localized.
///
/// `id` is an opaque application-defined field name. The bridge preserves
/// hand-built entries without validating or certifying them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RendererField {
    /// Stable, application-defined output field identifier.
    pub id: String,
    /// Whether the field's value may be localized.
    #[serde(default)]
    pub localization: RendererFieldLocalization,
}

/// Declared terminal sensitivity of stdout.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtyPolicy {
    /// Output is the same whether or not stdout is a terminal.
    Independent,
    /// Output adapts to whether stdout is a terminal.
    Adapts,
    /// Output cannot be produced without a terminal.
    Requires,
    /// Unrecognized future token.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Declared behaviour when stdin is closed.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClosedStdinPolicy {
    /// Stdin is unused.
    Ignored,
    /// A closed stdin is treated as end-of-file.
    Eof,
    /// A closed stdin is an error.
    Error,
    /// Unrecognized future token.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Declared policy for a presentation feature that can be off, automatic, or on.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TogglePolicy {
    /// The feature is never used.
    Never,
    /// The application selects whether to use the feature.
    Auto,
    /// The feature is always used.
    Always,
    /// Unrecognized future token.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Declared output width.
///
/// An unrecognized kind becomes `{"kind":"unknown"}` and its extra fields are
/// ignored. A known `fixed` kind with a missing, zero, or out-of-range column
/// count is rejected.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RenderWidth {
    /// Width follows the terminal.
    Terminal,
    /// Output is not constrained to a width.
    Unbounded,
    /// Output declares a fixed positive column count.
    Fixed {
        /// Declared column count; never zero.
        columns: NonZeroU16,
    },
    /// Unrecognized future width kind.
    #[default]
    Unknown,
}

impl<'de> Deserialize<'de> for RenderWidth {
    /// Reads the tagged width object, normalizing an unknown kind to
    /// [`RenderWidth::Unknown`].
    ///
    /// A `fixed` object must carry a positive column count that fits in
    /// `u16`; anything else is a hard error rather than a silent zero width.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// Structural view of the width object, so an unknown kind is not an
        /// error and a bad `fixed` payload still is.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum WidthObject {
            /// Width follows the terminal.
            Terminal,
            /// Output is not constrained to a width.
            Unbounded,
            /// Output declares a fixed positive column count.
            Fixed {
                /// Declared column count.
                columns: NonZeroU16,
            },
            /// Any unrecognized kind; extra fields are deliberately ignored.
            #[serde(other)]
            Unknown,
        }

        Ok(match WidthObject::deserialize(deserializer)? {
            WidthObject::Terminal => Self::Terminal,
            WidthObject::Unbounded => Self::Unbounded,
            WidthObject::Fixed { columns } => Self::Fixed { columns },
            WidthObject::Unknown => Self::Unknown,
        })
    }
}

/// Declared accessibility claim.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilityPolicy {
    /// No accessibility claim is made.
    #[default]
    Unspecified,
    /// Meaning does not depend solely on colour, emoji, animation, or layout.
    TextEquivalent,
    /// Unrecognized future token.
    #[serde(other)]
    Unknown,
}

/// Declared availability of plain, undecorated output.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlainPolicy {
    /// No plain-output mode exists.
    Unsupported,
    /// A plain-output mode exists.
    Available,
    /// Output is always plain.
    Always,
    /// Unrecognized future token.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Declared content category written to a stream.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamContent {
    /// Nothing is written.
    None,
    /// Human-readable text is written.
    Text,
    /// Machine-parseable structured content is written.
    Structured,
    /// Both human-readable and structured content are written.
    Mixed,
    /// Unrecognized future token.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Declared inheritance permission for downstream output.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InheritancePolicy {
    /// Downstream output may pass through.
    Allowed,
    /// Downstream output must not pass through.
    Forbidden,
    /// Unrecognized future token.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Whether an output field's value may be localized.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RendererFieldLocalization {
    /// The field's value may change with the human language.
    Localized,
    /// The field's value is protocol data and must not change with the language.
    NonLocalized,
    /// Unrecognized future classification, which makes no localization
    /// guarantee.
    #[default]
    #[serde(other)]
    Unknown,
}
