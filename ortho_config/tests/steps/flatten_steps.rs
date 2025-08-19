//! Steps for scenarios involving flattened CLI structures.

use crate::{FlatArgs, World};
use clap::Parser;
use cucumber::{given, then, when};
use figment::{Figment, providers::Serialized};
use ortho_config::{OrthoError, load_config_file, sanitized_provider};
use std::path::Path;

#[expect(
    clippy::result_large_err,
    reason = "test helper returns library error type"
)]
fn load_flat(file: Option<&str>, args: &[&str]) -> Result<FlatArgs, OrthoError> {
    let mut res = None;
    figment::Jail::expect_with(|j| {
        if let Some(contents) = file {
            j.create_file(".flat.toml", contents)?;
        }
        let cli = FlatArgs::parse_from(args);
        let mut fig = Figment::from(Serialized::defaults(&FlatArgs::default()));
        if let Some(f) = load_config_file(Path::new(".flat.toml"))? {
            fig = fig.merge(f);
        }
        res = Some(
            fig.merge(sanitized_provider(&cli)?)
                .extract()
                .map_err(OrthoError::merge),
        );
        Ok(())
    });
    res.expect("result")
}

#[given(expr = "the flattened configuration file has value {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn flattened_file(world: &mut World, val: String) {
    world.flat_file = Some(format!("nested = {{ value = \"{val}\" }}"));
}

#[given("a malformed flattened configuration file")]
fn malformed_flat_file(world: &mut World) {
    world.flat_file = Some("nested = 5".into());
}

#[when("the flattened config is loaded without CLI overrides")]
fn load_without_cli(world: &mut World) {
    world.flat_result = Some(load_flat(world.flat_file.as_deref(), &["prog"]));
}

#[when(expr = "the flattened config is loaded with CLI value {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn load_with_cli(world: &mut World, cli: String) {
    world.flat_result = Some(load_flat(
        world.flat_file.as_deref(),
        &["prog", "--value", &cli],
    ));
}

#[then(expr = "the flattened value is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn check_flattened(world: &mut World, expected: String) {
    let cfg = world.flat_result.take().expect("result").expect("ok");
    assert_eq!(cfg.nested.value.as_deref(), Some(expected.as_str()));
}

#[then("flattening fails with a merge error")]
fn flattening_fails(world: &mut World) {
    let err = world
        .flat_result
        .take()
        .expect("result")
        .expect_err("expected merge error");
    assert!(matches!(err, OrthoError::Merge { .. }));
}
