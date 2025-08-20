//! Steps demonstrating a renamed configuration path flag.

use crate::{RulesConfig, World};
use cucumber::{given, when};
use ortho_config::OrthoConfig;

#[given(expr = "an alternate config file with rule {string}")]
fn alt_config_file(world: &mut World, val: String) {
    world.file_value = Some(val);
}

#[when(expr = "the config is loaded with custom flag {string} {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn load_with_custom_flag(world: &mut World, flag: String, path: String) {
    let file_val = world.file_value.clone().expect("file value");
    let mut result = None;
    figment::Jail::expect_with(|j| {
        j.create_file(&path, &format!("rules = [\"{file_val}\"]"))?;
        result = Some(RulesConfig::load_from_iter(["prog", &flag, &path]));
        Ok(())
    });
    world.result = result;
}
