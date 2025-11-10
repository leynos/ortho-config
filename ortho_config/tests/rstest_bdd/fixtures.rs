//! Shared fixtures for the `rstest-bdd` behavioural scaffolding.

use ortho_config::OrthoConfig;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, OrthoConfig)]
pub struct CanaryConfig {
    #[ortho_config(default = 7)]
    pub level: u8,
}

#[derive(Debug, Default, ScenarioState)]
pub struct CanaryState {
    pub loaded_config: Slot<CanaryConfig>,
}

/// Creates a clean canary state so steps can share loaded configs.
#[fixture]
pub fn canary_state() -> CanaryState {
    CanaryState::default()
}

/// Provides the logical binary name used when constructing CLI args.
#[fixture]
pub fn binary_name() -> &'static str {
    "ortho-config-bdd"
}
