//! Steps verifying CLI precedence over environment variables and files.
#![allow(
    unfulfilled_lint_expectations,
    reason = "clippy::expect_used is denied globally; tests may not hit those branches"
)]
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[given(expr = "the configuration file has rules {string}")]
fn file_rules(world: &mut World, val: String) {
    world.file_value = Some(val);
}

#[when(expr = "the config is loaded with CLI rules {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn load_with_cli(world: &mut World, cli: String) {
    let file_val = world.file_value.clone();
    let env_val = world.env_value.clone();
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if let Some(f) = file_val {
            j.create_file(".ddlint.toml", &format!("rules = [\"{f}\"]"))?;
        }
        if let Some(e) = env_val {
            j.set_env("DDLINT_RULES", &e);
        }
        result = Some(RulesConfig::load_from_iter(["prog", "--rules", &cli]));
        Ok(())
    });
    world.result = result;
}

#[then(expr = "the loaded rules are {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn loaded_rules(world: &mut World, expected: String) {
    let cfg = world.result.take().expect("result").expect("ok");
    assert_eq!(cfg.rules.last().expect("rule"), &expected);
}
