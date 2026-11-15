//! Declared human and machine rendering carried in the documentation IR.
//!
//! These types let an author state how a command presents its output instead of
//! leaving consumers to infer it from command names, flags, output-format
//! names, or the host locale. Every field is optional, and the IR deliberately
//! has no `Unknown` variant: an undeclared policy is expressed by the
//! corresponding `Option` being `None`, and only the agent-context layer models
//! the `unknown` wire state.
//!
//! See ADR-0016 for the accepted vocabulary and defaulting contract.

use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};

/// Declared rendering for one command.
///
/// A present block with both records absent is equivalent to no declaration at
/// all. Records are never inherited by child commands: each command declares
/// its own rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererMetadata {
    /// Human-facing rendering description; `None` means undeclared.
    #[serde(default)]
    pub human: Option<RendererRecord>,
    /// Machine-facing rendering description; `None` means undeclared.
    #[serde(default)]
    pub machine: Option<RendererRecord>,
}

/// One renderer's declared presentation.
///
/// Every field is optional so a partial declaration stays partial. Missing
/// support is distinct from an explicit `false`; the compatibility defaults
/// belong to the agent-context layer, not here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererRecord {
    /// Whether this renderer is supported; `None` means undeclared.
    #[serde(default)]
    pub supported: Option<bool>,
    /// Terminal (TTY) sensitivity of stdout; `None` means undeclared.
    #[serde(default)]
    pub tty: Option<TtyPolicy>,
    /// Behaviour when stdin is closed; `None` means undeclared.
    #[serde(default)]
    pub closed_stdin: Option<ClosedStdinPolicy>,
    /// Colour policy; `None` means undeclared.
    #[serde(default)]
    pub colour: Option<TogglePolicy>,
    /// Emoji policy; `None` means undeclared.
    #[serde(default)]
    pub emoji: Option<TogglePolicy>,
    /// Progress-animation policy; `None` means undeclared.
    #[serde(default)]
    pub progress: Option<TogglePolicy>,
    /// Pager policy; `None` means undeclared.
    #[serde(default)]
    pub pager: Option<TogglePolicy>,
    /// Declared output width; `None` means undeclared.
    #[serde(default)]
    pub width: Option<RenderWidth>,
    /// Accessibility claim; `None` means undeclared.
    #[serde(default)]
    pub accessibility: Option<AccessibilityPolicy>,
    /// Plain-output availability; `None` means undeclared.
    #[serde(default)]
    pub plain: Option<PlainPolicy>,
    /// Declared stdout content category; `None` means undeclared.
    #[serde(default)]
    pub stdout: Option<StreamContent>,
    /// Declared stderr content category; `None` means undeclared.
    #[serde(default)]
    pub stderr: Option<StreamContent>,
    /// Whether progress output may pass through; `None` means undeclared.
    #[serde(default)]
    pub progress_inheritance: Option<InheritancePolicy>,
    /// Whether subprocess output may pass through; `None` means undeclared.
    #[serde(default)]
    pub subprocess_inheritance: Option<InheritancePolicy>,
    /// Output-field localization claims in declaration order.
    ///
    /// Defaults to an empty list, which makes no claim about undeclared
    /// fields, and is omitted from the wire when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<RendererField>,
}

/// One output field and whether its value may be localized.
///
/// `id` is an opaque application-defined field name, not a JSON pointer or an
/// executable lookup. The classification describes the field's value, never
/// its identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererField {
    /// Stable, application-defined output field identifier.
    pub id: String,
    /// Whether the field's value may be localized.
    pub localization: RendererFieldLocalization,
}

/// Declared terminal sensitivity of stdout.
///
/// A terminal here means stdout's terminal status; it says nothing about stdin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtyPolicy {
    /// Output is the same whether or not stdout is a terminal.
    Independent,
    /// Output adapts to whether stdout is a terminal.
    Adapts,
    /// Output cannot be produced without a terminal.
    Requires,
}

/// Declared behaviour when stdin is closed.
///
/// No exit class is implied by any variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedStdinPolicy {
    /// Stdin is unused.
    Ignored,
    /// A closed stdin is treated as end-of-file.
    Eof,
    /// A closed stdin is an error.
    Error,
}

/// Declared policy for a presentation feature that can be off, automatic, or on.
///
/// `Auto` means application-selected; it is not a promise about a particular
/// environment variable or precedence algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TogglePolicy {
    /// The feature is never used.
    Never,
    /// The application selects whether to use the feature.
    Auto,
    /// The feature is always used.
    Always,
}

/// Declared output width.
///
/// Serialized as an internally tagged object: `{"kind":"terminal"}`,
/// `{"kind":"unbounded"}`, or `{"kind":"fixed","columns":80}`. The fixed variant
/// carries a non-zero column count so a declaration cannot contradict itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// Declared accessibility claim.
///
/// This is an application claim, not an accessibility certification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilityPolicy {
    /// No accessibility claim is made.
    Unspecified,
    /// Meaning does not depend solely on colour, emoji, animation, or layout.
    TextEquivalent,
}

/// Declared availability of plain, undecorated output.
///
/// `Available` asserts that a mode exists; it does not add a flag or resolve
/// option precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlainPolicy {
    /// No plain-output mode exists.
    Unsupported,
    /// A plain-output mode exists.
    Available,
    /// Output is always plain.
    Always,
}

/// Declared content category written to a stream.
///
/// These describe content categories, not framing or document counts.
/// Structured output need not be JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// Declared inheritance permission for downstream output.
///
/// Inheritance means downstream output may pass through directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InheritancePolicy {
    /// Downstream output may pass through.
    Allowed,
    /// Downstream output must not pass through.
    Forbidden,
}

/// Whether an output field's value may be localized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererFieldLocalization {
    /// The field's value may change with the human language.
    Localized,
    /// The field's value is protocol data and must not change with the language.
    NonLocalized,
}
