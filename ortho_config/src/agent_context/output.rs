//! Structured-output contracts for agent context (roadmap 7.2.3).
//!
//! Split from `mod` so each module stays beneath the 400-line cap. These types
//! are the tolerant reading side of the output metadata: unlike the
//! documentation IR, the stdout enum carries an `Unknown` fallback so a
//! consumer that meets a future token keeps reading the rest of the document.
//!
//! Every member keeps the author's declared state. In particular
//! `supports_json` stays an `Option` rather than defaulting to `false`, so an
//! agent can tell "the author refused JSON" from "the author said nothing".
//!
//! See ADR-0020 for the vocabulary, the omission rules, and the no-inference
//! contract.

use serde::{Deserialize, Serialize};

/// Declared structured-output capability for one command in agent context.
///
/// Serialization omits the block entirely when a command declares nothing, and
/// omits every absent member inside a present block, matching the renderer
/// contract's omission convention. A present empty block still makes no
/// capability claim.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputContract {
    /// Whether the command offers JSON output; `None` means undeclared.
    ///
    /// This is the author's assertion that a `--json` capability exists. It is
    /// not an observation that any particular flag, runtime, or payload exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_json: Option<bool>,
    /// Purpose of the normal successful result on stdout; `None` is undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<StdoutPurpose>,
    /// Whether stderr can carry diagnostics; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_diagnostics: Option<bool>,
    /// Documented exit classes in the author's declared order.
    ///
    /// Omitted when empty. An empty list documents no class; it never asserts
    /// that the command cannot fail.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit_classes: Vec<ExitClass>,
}

/// Purpose of a command's normal successful result on stdout.
///
/// The tolerant wire enum: an unrecognized future token reads as
/// [`StdoutPurpose::Unknown`], which establishes no data-returning status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StdoutPurpose {
    /// Successful output is the command's data result.
    Data,
    /// Successful output is a filesystem path the command produced.
    Path,
    /// Successful output is a human-readable summary of work done.
    Summary,
    /// A successful invocation writes nothing to stdout.
    None,
    /// Unrecognized future token; never treated as data-returning.
    #[default]
    #[serde(other)]
    Unknown,
}

/// One documented exit class.
///
/// `name` is a stable protocol identifier and `meaning` a verbatim explanation.
/// The bridge copies both without validating, sorting, or localizing them; the
/// derive is what rejects a malformed declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitClass {
    /// Stable, non-localized identifier.
    pub name: String,
    /// Explanation of when the class applies, carried verbatim.
    pub meaning: String,
}
