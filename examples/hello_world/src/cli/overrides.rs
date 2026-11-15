//! Data structures that capture layered configuration overrides.
//!
//! These types exist solely to bridge between figment providers and the
//! higher-level CLI structures, keeping serialization concerns isolated from
//! command parsing logic.

use serde::{Deserialize, Serialize};

use crate::cli::is_false;

/// Serializable CLI-only fields appended as the highest-precedence layer.
///
/// Optional values are omitted when absent, and false switches are omitted so
/// an unspecified CLI flag does not erase a lower-precedence `true` value.
#[derive(Serialize)]
pub(crate) struct Overrides<'a> {
    /// Borrowed recipient, omitted unless explicitly set on the CLI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recipient: Option<&'a String>,
    /// Replacement salutation list, omitted when the CLI supplied none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) salutations: Option<Vec<String>>,
    /// Explicit enabled value; false is omitted to preserve merged settings.
    #[serde(skip_serializing_if = "is_false")]
    pub(crate) is_excited: bool,
    /// Explicit enabled value; false is omitted to preserve merged settings.
    #[serde(skip_serializing_if = "is_false")]
    pub(crate) is_quiet: bool,
}

/// File-backed global and nested command overrides deserialized from a layer.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct FileOverrides {
    /// Optional file setting for the global enthusiastic mode.
    #[serde(default)]
    pub(crate) is_excited: Option<bool>,
    /// Command-specific settings keyed under the serialized `cmds` table.
    #[serde(default)]
    pub(crate) cmds: CommandOverrides,
}

/// Command-specific section of the file override document.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct CommandOverrides {
    /// Optional `greet` settings; absence leaves greeting defaults untouched.
    #[serde(default)]
    pub(crate) greet: Option<GreetOverrides>,
}

/// File overrides applied after greeting defaults are loaded.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct GreetOverrides {
    /// Optional preamble override; `None` means the file supplied no value.
    #[serde(default)]
    pub(crate) preamble: Option<String>,
    /// Optional punctuation override with the same absent-versus-value distinction.
    #[serde(default)]
    pub(crate) punctuation: Option<String>,
}
