//! Cucumber step definitions for testing subcommand configuration loading and
//! merging behaviour.
//!
//! This module provides step definitions that verify the correct precedence and
//! merging of configuration sources (CLI arguments, environment variables, and
//! configuration files) when loading subcommand configurations.

use crate::{PrArgs, World};
use clap::Parser;
use cucumber::{given, then, when};
use ortho_config::SubcmdConfigMerge;

/// Check if all configuration sources are absent.
fn has_no_config_sources(world: &World) -> bool {
    world.sub_ref.is_none() && world.sub_file.is_none() && world.sub_env.is_none()
}

#[given(expr = "a CLI reference {string}")]
fn set_cli_ref(world: &mut World, val: String) {
    world.sub_ref = Some(val);
}

#[given("no CLI reference")]
fn no_cli_ref(world: &mut World) {
    world.sub_ref = None;
}

#[given(expr = "a configuration reference {string}")]
fn file_ref(world: &mut World, val: String) {
    world.sub_file = Some(val);
}

#[given(expr = "an environment reference {string}")]
fn env_ref(world: &mut World, val: String) {
    world.sub_env = Some(val);
}

#[when("the subcommand configuration is loaded without defaults")]
fn load_sub(world: &mut World) {
    let result = if has_no_config_sources(world) {
        PrArgs::try_parse_from(["test"]).map_err(|e| ortho_config::OrthoError::from(e).into())
    } else {
        let cli = PrArgs {
            reference: world.sub_ref.clone(),
        };
        setup_test_environment(world, &cli)
    };
    world.sub_result = Some(result);
    world.sub_file = None;
    world.sub_env = None;
}

/// Set up test environment with configuration file and environment variables.
fn setup_test_environment(world: &World, cli: &PrArgs) -> ortho_config::OrthoResult<PrArgs> {
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if let Some(ref val) = world.sub_file {
            j.create_file(".app.toml", &format!("[cmds.test]\nreference = \"{val}\""))?;
        }
        if let Some(ref val) = world.sub_env {
            j.set_env("APP_CMDS_TEST_REFERENCE", val);
        }
        result = Some(cli.load_and_merge());
        Ok(())
    });
    result.expect("jail setup should complete")
}

#[then(expr = "the merged reference is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn check_ref(world: &mut World, expected: String) {
    let cfg = world.sub_result.take().expect("result").expect("ok");
    assert_eq!(cfg.reference.as_deref(), Some(expected.as_str()));
}

#[then("the subcommand load fails")]
fn sub_error(world: &mut World) {
    let result = world.sub_result.take().expect("result");
    match result {
        Err(_) => {}
        Ok(_) => panic!("Expected subcommand load to fail, but it succeeded"),
    }
}
