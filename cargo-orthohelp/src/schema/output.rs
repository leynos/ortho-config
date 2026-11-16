//! Local mirror of the documentation IR's structured-output declarations.
//!
//! `cargo-orthohelp` parses IR written by the derive macro, so it repeats the
//! output shape rather than depending on unpublished crate internals. An
//! undeclared aspect is an absent `Option`, matching the authoring side; the
//! tolerant `unknown` state exists only in agent context. The shape must
//! serialize exactly as `ortho_config::docs::OutputMetadata` does.

use serde::{Deserialize, Serialize};

/// Declared structured-output capability for one command.
///
/// A present block with every member absent is equivalent to no declaration at
/// all. Capability is never inherited by child commands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputMetadata {
    /// Whether the command offers JSON output; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_json: Option<bool>,
    /// Purpose of the normal successful result on stdout; `None` is undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<StdoutPurpose>,
    /// Whether stderr can carry diagnostics; `None` means undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_diagnostics: Option<bool>,
    /// Documented exit classes in declaration order; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit_classes: Vec<ExitClass>,
}

/// Purpose of a command's normal successful result on stdout.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitClass {
    /// Stable, non-localized identifier matching `[a-z][a-z0-9_]*`.
    pub name: String,
    /// Nonblank explanation of when the class applies.
    pub meaning: String,
}
