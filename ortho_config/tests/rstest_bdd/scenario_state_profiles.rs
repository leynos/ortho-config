//! Profile scenario state and fixtures (`profiles.feature`).
//!
//! Split from `scenario_state` so each module stays beneath the 400-line
//! cap. The steps import the state via the `scenario_state` re-exports.

use ortho_config::{OrthoConfig, ProfileSource, SelectedProfile};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};

/// Scenario state for profile selection and layering scenarios
/// (`profiles.feature`, milestones 3–4).
#[derive(Debug, Default, ScenarioState)]
pub struct ProfilesContext {
    /// Base file keys as `(key, value)` pairs.
    pub base_keys: Slot<Vec<(String, String)>>,
    /// Profile tables as `(profile, key, value)` triples; an empty key means
    /// an empty profile table.
    pub profile_keys: Slot<Vec<(String, String, String)>>,
    /// The `APP_PROFILE` selector value.
    pub selector_env: Slot<String>,
    /// Environment overrides as `(key, value)` pairs using the `APP_` prefix.
    pub env_keys: Slot<Vec<(String, String)>>,
    /// Struct default for the merged key (flag-equals-default scenario).
    pub struct_default: Slot<String>,
    /// Whether no config file should be discoverable.
    pub no_files: Slot<()>,
    /// Load result.
    pub result: Slot<ortho_config::OrthoResult<ProfilesConfig>>,
    /// Selection result.
    pub selection: Slot<Vec<SelectedProfile>>,
    /// Structured fields of the last `UnknownProfile` error.
    pub error_selected: Slot<String>,
    pub error_source: Slot<ProfileSource>,
    pub error_available: Slot<Vec<String>>,
    /// Structured fields of the last `ProfileForbiddenKey` error.
    pub error_profile: Slot<String>,
    pub error_key: Slot<String>,
    /// Rendered message of any load error.
    pub error_message: Slot<String>,
}

/// Provides a clean profile context for profile selection scenarios.
#[fixture]
pub fn profiles_context() -> ProfilesContext {
    ProfilesContext::default()
}

/// Configuration struct used by profile BDD scenarios.
///
/// The `APP_` prefix gives the `APP_PROFILE` selector variable. The derive is
/// not profile-opted-in; the profile steps drive the library helpers directly
/// until the opt-in derive lands (milestone 4).
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "APP_")]
pub struct ProfilesConfig {
    #[serde(default)]
    pub retries: u32,
}
