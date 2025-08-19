//! Steps for scenarios involving flattened CLI structures.

use crate::{FlatArgs, World};
use clap::Parser;
use cucumber::{given, then, when};
use figment::{Figment, providers::Serialized};
use ortho_config::{OrthoError, load_config_file, sanitized_provider};
use std::path::Path;

#[given(expr = "the flattened configuration file has value {string}")]
fn flattened_file(world: &mut World, val: String) {
    world.flat_file = Some(val);
}

#[when("the flattened config is loaded without CLI overrides")]
fn load_without_cli(world: &mut World) {
    let file_val = world.flat_file.clone();
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if let Some(v) = file_val {
            j.create_file(".flat.toml", &format!("nested = {{ value = \"{v}\" }}"))?;
        }
        result = Some((|| -> Result<FlatArgs, OrthoError> {
            let cli = FlatArgs::parse_from(["prog"]);
            let mut fig = Figment::from(Serialized::defaults(&FlatArgs::default()));
            if let Some(f) = load_config_file(Path::new(".flat.toml"))? {
                fig = fig.merge(f);
            }
            fig.merge(sanitized_provider(&cli)?)
                .extract()
                .map_err(OrthoError::from)
        })());
        Ok(())
    });
    world.flat_result = result;
}

#[when(expr = "the flattened config is loaded with CLI value {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
fn load_with_cli(world: &mut World, cli: String) {
    let file_val = world.flat_file.clone();
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if let Some(v) = file_val {
            j.create_file(".flat.toml", &format!("nested = {{ value = \"{v}\" }}"))?;
        }
        result = Some((|| -> Result<FlatArgs, OrthoError> {
            let cli = FlatArgs::parse_from(["prog", "--value", &cli]);
            let mut fig = Figment::from(Serialized::defaults(&FlatArgs::default()));
            if let Some(f) = load_config_file(Path::new(".flat.toml"))? {
                fig = fig.merge(f);
            }
            fig.merge(sanitized_provider(&cli)?)
                .extract()
                .map_err(OrthoError::from)
        })());
        Ok(())
    });
    world.flat_result = result;
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
