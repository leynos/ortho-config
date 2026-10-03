//! Step definitions backing the canary `rstest-bdd` scenario.

#[cfg(test)]
use super::scenario_state::canary_state;
use super::scenario_state::{CanaryConfig, CanaryState};
use anyhow::{Context, Result, anyhow, ensure};
use ortho_config::OrthoConfig;
#[cfg(test)]
use rstest::rstest;
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
fn load_canary(canary_state: &CanaryState, binary_name: &str, level: u8) -> Result<()> {
    let args = vec![
        binary_name.to_string(),
        "--level".to_string(),
        level.to_string(),
    ];
    load_canary_from_args(canary_state, args)
}

fn load_canary_from_args(canary_state: &CanaryState, args: Vec<String>) -> Result<()> {
    let config = CanaryConfig::load_from_iter(args)
        .context("canary configuration should load successfully")?;
    canary_state.loaded_config.set(config);
    Ok(())
}

#[rstest]
fn invalid_canary_level_preserves_error_chain(canary_state: CanaryState) {
    let result = load_canary_from_args(
        &canary_state,
        vec![
            "ortho-config-bdd".to_owned(),
            "--level".to_owned(),
            "not-a-number".to_owned(),
        ],
    );
    assert!(result.is_err(), "invalid canary level should be rejected");
    let Err(error) = result else {
        return;
    };
    let error_chain = error.chain().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(
        error_chain.first().map(String::as_str),
        Some("canary configuration should load successfully")
    );
    assert!(
        error_chain
            .iter()
            .skip(1)
            .any(|message| message.contains("not-a-number")),
        "original parser error should remain in the chain: {error_chain:?}"
    );
    assert!(canary_state.loaded_config.is_empty());
}

#[then("the canary level equals {expected:u8}")]
fn assert_level(canary_state: &CanaryState, expected: u8) -> Result<()> {
    let actual = canary_state
        .loaded_config
        .with_ref(|cfg| cfg.level)
        .ok_or_else(|| anyhow!("a canary config must have been stored"))?;
    ensure!(
        actual == expected,
        "actual level {actual}; expected {expected}"
    );
    Ok(())
}
