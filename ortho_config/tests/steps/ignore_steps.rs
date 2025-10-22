//! Steps for testing ignore pattern list handling.
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

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
    let env_val = world.env_value.take();
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if let Some(val) = env_val.as_deref() {
            j.set_env("DDLINT_IGNORE_PATTERNS", val);
        }
        let mut args = vec!["prog".to_string()];
        if !cli.is_empty() {
            args.push("--ignore-patterns".into());
            args.push(cli.trim().to_string());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        result = Some(<RulesConfig as ortho_config::OrthoConfig>::load_from_iter(
            refs,
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
