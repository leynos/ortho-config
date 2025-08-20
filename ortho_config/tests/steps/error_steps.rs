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

#[then("CLI, file and environment errors are returned")]
fn cli_file_env_errors(world: &mut World) {
    let err = world
        .agg_result
        .take()
        .expect("missing test result")
        .expect_err("expected aggregated error");
    match err {
        ortho_config::OrthoError::Aggregate(ref agg) => {
            let mut saw_cli = false;
            let mut saw_file = false;
            let mut saw_env = false;
            for e in agg.iter() {
                match e {
                    ortho_config::OrthoError::CliParsing(_) => saw_cli = true,
                    ortho_config::OrthoError::File { .. } => saw_file = true,
                    ortho_config::OrthoError::Merge { .. }
                    | ortho_config::OrthoError::Gathering(_) => saw_env = true,
                    _ => {}
                }
            }
            assert!(saw_cli, "expected CLI parsing error in aggregate");
            assert!(saw_file, "expected file error in aggregate");
            assert!(saw_env, "expected environment error in aggregate");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[then("a CLI parsing error is returned")]
fn cli_error_only(world: &mut World) {
    let err = world
        .agg_result
        .take()
        .expect("missing test result")
        .expect_err("expected CLI parsing error");
    match err {
        ortho_config::OrthoError::CliParsing(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}
