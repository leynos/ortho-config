//! Steps for testing ignore pattern list handling.

use crate::{RulesConfig, World};
use cucumber::{given, then, when};

#[given(expr = "the environment variable DDLINT_IGNORE_PATTERNS is {string}")]
fn set_ignore_env(world: &mut World, val: String) {
    world.env_value = Some(val);
}

#[when(expr = "the config is loaded with CLI ignore {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn load_ignore(world: &mut World, cli: String) {
    let env_val = world
        .env_value
        .as_deref()
        .expect("DDLINT_IGNORE_PATTERNS not set");
    let mut result = None;
    figment::Jail::expect_with(|j| {
        j.set_env("DDLINT_IGNORE_PATTERNS", env_val);
        result = Some(<RulesConfig as ortho_config::OrthoConfig>::load_from_iter(
            ["prog", "--ignore-patterns", cli.as_str()],
        ));
        Ok(())
    });
    world.result = result;
}

#[then(expr = "the ignore patterns are {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn check_ignore(world: &mut World, expected: String) {
    let cfg = world.result.take().expect("result").expect("ok");
    let want: Vec<String> = expected.split(',').map(|s| s.trim().to_string()).collect();
    assert_eq!(cfg.ignore_patterns, want);
}
