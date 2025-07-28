use crate::{RulesConfig, World};
use clap::Parser;
use cucumber::{given, then, when};

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
        let cli = RulesConfig::parse_from(["prog"]);
        result = Some(cli.load_and_merge());
        Ok(())
    });
    world.result = result;
}

#[then(expr = "the rules are {string}")]
#[allow(clippy::needless_pass_by_value)]
fn check_rules(world: &mut World, expected: String) {
    let cfg = world.result.take().expect("result").expect("ok");
    let want: Vec<String> = expected.split(',').map(str::to_string).collect();
    assert_eq!(cfg.rules, want);
}
