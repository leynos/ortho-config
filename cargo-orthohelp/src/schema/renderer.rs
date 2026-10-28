//! Local mirror of the documentation IR's renderer declarations.
//!
//! `cargo-orthohelp` parses IR written by the derive macro, so it repeats the
//! renderer shape rather than depending on unpublished crate internals. An
//! undeclared policy is an absent `Option`, matching the authoring side; the
//! tolerant `unknown` states exist only in agent context.

use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};

/// Declared rendering for one command.
///
/// A present block with both records absent is equivalent to no declaration at
/// all. Records are never inherited by child commands.
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
/// support is distinct from an explicit `false`.
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<RendererField>,
}

/// One output field and whether its value may be localized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererField {
    /// Stable, application-defined output field identifier.
    pub id: String,
    /// Whether the field's value may be localized.
    pub localization: RendererFieldLocalization,
}

/// Declared terminal sensitivity of stdout.
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
/// Serialized as an internally tagged object; the fixed variant carries a
/// non-zero column count so a declaration cannot contradict itself.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilityPolicy {
    /// No accessibility claim is made.
    Unspecified,
    /// Meaning does not depend solely on colour, emoji, animation, or layout.
    TextEquivalent,
}

/// Declared availability of plain, undecorated output.
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
