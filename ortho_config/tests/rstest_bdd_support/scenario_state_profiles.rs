//! Profile scenario state and fixtures (`profiles.feature`).
//!
//! Split from `scenario_state` so each module stays beneath the 400-line
//! cap. The steps import the state via the `scenario_state` re-exports.

use ortho_config::{MergeLayer, OrthoConfig, ProfileLoadOutcome, ProfileSource, SelectedProfile};
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
    pub result: Slot<ortho_config::OrthoResult<ProfileLoadOutcome<ProfilesConfig>>>,
    /// Composed layers recorded for the selector-never-leaks assertion.
    pub layers: Slot<Vec<MergeLayer<'static>>>,
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
    /// Every error the load retained, in the order they are reported. An
    /// aggregated failure records one entry per sub-error; a single failure
    /// records that one error.
    pub error_variants: Slot<Vec<String>>,
}

/// Provides a clean profile context for profile selection scenarios.
#[fixture]
pub fn profiles_context() -> ProfilesContext {
    ProfilesContext::default()
}

/// Configuration struct used by profile BDD scenarios.
///
/// The `APP_` prefix gives the `APP_PROFILE` selector variable. The struct is
/// profile-opted-in so the scenarios exercise the generated `--profile` flag
/// and `load_with_profile` entry point. The `retries` default of 3 drives the
/// flag-equals-default scenario: an explicit `--retries 3` equals the default
/// and must still beat the profile.
///
/// `verbose` is a `bool`, so the generated CLI emits it with
/// [`clap::ArgAction::SetTrue`]. An absent `--verbose` therefore reaches clap as
/// a real `false` rather than an unset `None`, which is what the two
/// bool-precedence scenarios pin: the spurious default must be pruned, while a
/// flag the operator actually typed must survive. Its declared default of
/// `false` is what makes the first scenario observable — a profile setting
/// `verbose = true` is exactly the case an unpruned `false` would silently
/// reset.
/// `strict` is the `cli_default_as_absent` probe. The attribute exists to stop
/// a clap-supplied default from outranking a file or environment value, which
/// is only observable when a value arrives from a layer beneath the CLI. The
/// scenario pairing a profile value with an `APP_STRICT` environment value
/// therefore pins the documented tier order for such a field: the environment
/// layer outranks the profile layer, and the CLI layer must not prune the
/// environment's contribution out from under it.
///
/// A `bool` is the probe's shape because `SetTrue` gives it a clap-supplied
/// default on the generated argument without needing `#[arg(...)]`, which this
/// fixture cannot carry: it derives only `OrthoConfig`, and only clap's own
/// derive registers that helper attribute.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "APP_", profiles)]
pub struct ProfilesConfig {
    #[ortho_config(default = 3)]
    pub retries: u32,
    #[ortho_config(default = false)]
    pub verbose: bool,
    #[ortho_config(default = false, cli_default_as_absent)]
    pub strict: bool,
}
