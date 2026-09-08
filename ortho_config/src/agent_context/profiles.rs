//! Profile support declarations (roadmap 9.1.1).
//!
//! Split from `mod` so each module stays beneath the 400-line cap.

use serde::{Deserialize, Serialize};

/// Profile support declared by an application (roadmap 9.1.1).
///
/// The unsupported case serializes byte-identically to the legacy
/// `{ "supported": false }` because the optional fields are omitted when
/// absent (decision D7). Prefer the constructors over struct literals so
/// downstream construction survives future field additions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfilesDeclaration {
    /// Whether the application supports profiles.
    pub supported: bool,
    /// The canonical selection contract (flag and environment variable names).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<ProfileSelectionContract>,
    /// Command path listing available profiles, populated by roadmap 9.1.3.
    ///
    /// Matches [`crate::agent_context::AgentCommand::path`] token-for-token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_command: Option<Vec<String>>,
}

impl ProfilesDeclaration {
    /// The legacy unsupported declaration.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            supported: false,
            selection: None,
            list_command: None,
        }
    }

    /// A supported declaration carrying the selection contract.
    #[must_use]
    pub const fn supported(selection: ProfileSelectionContract) -> Self {
        Self {
            supported: true,
            selection: Some(selection),
            list_command: None,
        }
    }
}

impl Default for ProfilesDeclaration {
    fn default() -> Self {
        Self::unsupported()
    }
}

/// The canonical profile selection contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionContract {
    /// The selector flag name following the `AgentInput::long` convention
    /// (no leading `--`).
    pub flag: String,
    /// The selector environment variable name (for example `APP_PROFILE`).
    pub env_var: String,
}
