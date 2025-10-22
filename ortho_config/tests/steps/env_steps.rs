//! Cucumber step definitions for environment variable testing.
//!
//! Provides BDD steps for setting environment variables, loading configuration
//! using [`CsvEnv`], and verifying parsed results.
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use crate::{RulesConfig, World};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

/// Sets `DDLINT_RULES` in the test environment.
#[given(expr = "the environment variable DDLINT_RULES is {string}")]
fn set_env(world: &mut World, val: String) {
    world.env_value = Some(val);
}

#[when("the configuration is loaded")]
fn load_config(world: &mut World) {
    let val = world.env_value.clone().expect("env value");
    let mut result = None;
    figment::Jail::expect_with(|j| {
        j.set_env("DDLINT_RULES", &val);
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    });
    world.result = result;
}

#[then(expr = "the rules are {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
/// Verifies that the parsed rule list matches the expected string.
fn check_rules(world: &mut World, expected: String) {
    let cfg = world.result.take().expect("result").expect("ok");
    let want: Vec<String> = expected.split(',').map(str::to_string).collect();
    assert_eq!(cfg.rules, want);
}
