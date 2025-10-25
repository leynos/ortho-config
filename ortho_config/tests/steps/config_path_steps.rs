//! Steps demonstrating a renamed configuration path flag.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[given(expr = "an alternate config file with rule {string}")]
fn alt_config_file(world: &mut World, val: String) -> Result<()> {
    ensure!(
        !val.trim().is_empty(),
        "alternate config rule must not be empty"
    );
    ensure!(
        world.file_value.is_none(),
        "alternate config file already initialised"
    );
    world.file_value = Some(val);
    Ok(())
}

#[when(expr = "the config is loaded with custom flag {string} {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn load_with_custom_flag(world: &mut World, flag: String, path: String) -> Result<()> {
    let file_val = world
        .file_value
        .take()
        .ok_or_else(|| anyhow!("alternate config file value not provided"))?;
    let mut result = None;
    figment::Jail::try_with(|j| {
        j.create_file(&path, &format!("rules = [\"{file_val}\"]"))?;
        let args = ["prog", flag.as_str(), path.as_str()];
        result = Some(RulesConfig::load_from_iter(args));
        Ok(())
    })
    .map_err(anyhow::Error::new)?;
    world.result = result;
    ensure!(
        world.result.is_some(),
        "configuration load did not produce a result"
    );
    Ok(())
}

#[then("config loading fails with a CLI parsing error")]
fn cli_error(world: &mut World) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    match result {
        Ok(_) => Err(anyhow!(
            "expected CLI parsing error but configuration succeeded"
        )),
        Err(err) => match err.as_ref() {
            ortho_config::OrthoError::CliParsing(_) => Ok(()),
            other => Err(anyhow!("unexpected error: {other:?}")),
        },
    }
}
