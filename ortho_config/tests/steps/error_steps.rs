//! Steps verifying aggregated error reporting.

use crate::{ErrorConfig, World};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[given("an invalid configuration file")]
fn invalid_file(world: &mut World) {
    world.file_value = Some("port = ".into());
}

#[given(expr = "the environment variable DDLINT_PORT is {string}")]
fn env_port(world: &mut World, val: String) {
    world.env_value = Some(val);
}

#[when("the config is loaded with an invalid CLI argument")]
fn load_invalid_cli(world: &mut World) {
    let file_val = world.file_value.clone();
    let env_val = world.env_value.clone();
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if let Some(f) = file_val {
            j.create_file(".ddlint.toml", &f)?;
        }
        if let Some(e) = env_val {
            j.set_env("DDLINT_PORT", &e);
        }
        result = Some(ErrorConfig::load_from_iter(["prog", "--bogus"]));
        Ok(())
    });
    world.agg_result = result;
}

#[then("multiple errors are returned")]
fn multiple_errors(world: &mut World) {
    let err = world.agg_result.take().expect("result").unwrap_err();
    match err {
        ortho_config::OrthoError::Aggregate(ref agg) => {
            assert!(agg.len() > 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
