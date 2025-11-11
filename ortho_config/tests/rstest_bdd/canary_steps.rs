//! Step definitions backing the canary `rstest-bdd` scenario.

use super::fixtures::{CanaryConfig, CanaryState};
use ortho_config::OrthoConfig;
use rstest_bdd::ScenarioState as _;
use rstest_bdd_macros::{given, then, when};

#[given("the canary scenario state is reset")]
fn reset_state(canary_state: &CanaryState) {
    canary_state.reset();
    assert!(
        canary_state.loaded_config.is_empty(),
        "reset must clear previously loaded configs"
    );
}

#[when("I load the canary config with level {level:u8}")]
fn load_canary(canary_state: &CanaryState, binary_name: &str, level: u8) {
    let args = vec![
        binary_name.to_string(),
        "--level".to_string(),
        level.to_string(),
    ];
    let config =
        CanaryConfig::load_from_iter(args).expect("canary configuration should load successfully");
    canary_state.loaded_config.set(config);
}

#[then("the canary level equals {expected:u8}")]
fn assert_level(canary_state: &CanaryState, expected: u8) {
    let actual = canary_state
        .loaded_config
        .with_ref(|cfg| cfg.level)
        .expect("a canary config must have been stored");
    assert_eq!(actual, expected);
}
