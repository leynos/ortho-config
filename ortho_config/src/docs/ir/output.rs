//! Declared structured-output capability and exit classes in the docs IR.
//!
//! These types let an author state whether a command offers JSON output, what
//! its normal successful result is, whether stderr carries diagnostics, and
//! which exit classes the command documents. They describe declarations, not
//! observed runtime behaviour: nothing here inspects a flag, a renderer, a
//! locale, or the terminal, and an absent declaration never becomes a claim.
//!
//! The IR deliberately has no `Unknown` variant. An undeclared aspect is the
//! corresponding `Option` being `None`, and only the agent-context layer models
//! the tolerant `unknown` wire state.
//!
//! See ADR-0020 for the accepted vocabulary, the omission rules, and the
//! no-inference contract. A later roadmap item (7.2.5) owns numeric exit-code
//! mappings, so [`ExitClass`] intentionally carries no code number.

use serde::{Deserialize, Serialize};

/// Declared structured-output capability for one command.
///
/// A present block with every member absent is representable as `{}` and makes
/// exactly the same capability claim as no block at all: none. Each command
/// declares its own capability; a parent's declaration is never inherited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputMetadata {
    /// Whether the command offers JSON output; `None` means undeclared.
    ///
    /// `Some(false)` and `None` both mean the capability is not claimed, but
    /// they stay distinguishable so a reader can tell a refusal from a silence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_json: Option<bool>,
    /// Purpose of the normal successful result on stdout; `None` is undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<StdoutPurpose>,
    /// Whether stderr can carry diagnostics; `None` means undeclared.
    ///
    /// The declaration concerns diagnostic output, not the normal result, and
    /// says nothing about exit status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_diagnostics: Option<bool>,
    /// Documented exit classes in declaration order.
    ///
    /// Defaults to an empty list, which documents no class rather than
    /// asserting that the command cannot fail, and is omitted from the wire
    /// when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit_classes: Vec<ExitClass>,
}

/// Purpose of a command's normal successful result on stdout.
///
/// This describes the semantic result only. It says nothing about presentation,
/// encoding, or whether the output is identical on every invocation, and it
/// excludes help, version, and error output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// One documented exit class.
///
/// `name` is a stable, non-localized protocol identifier; `meaning` is a
/// human-readable explanation carried verbatim and never resolved as a Fluent
/// key. Neither member implies a numeric status: code-to-class mapping is
/// roadmap 7.2.5 work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitClass {
    /// Stable, non-localized identifier matching `[a-z][a-z0-9_]*`.
    pub name: String,
    /// Nonblank explanation of when the class applies.
    pub meaning: String,
}
