//! Steps for testing ignore pattern list handling.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};

#[given(expr = "the environment variable DDLINT_IGNORE_PATTERNS is {string}")]
fn set_ignore_env(world: &mut World, env_value: String) -> Result<()> {
    ensure!(
        world.env_value.is_none(),
        "ignore patterns environment value already initialised"
    );
    world.env_value = Some(env_value);
    Ok(())
}

#[when(expr = "the config is loaded with CLI ignore {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn load_ignore(world: &mut World, cli_arg: String) -> Result<()> {
    let env_val = world.env_value.take();
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(val) = env_val.as_deref() {
            j.set_env("DDLINT_IGNORE_PATTERNS", val);
        }
        let mut args = vec![String::from("prog")];
        if !cli_arg.is_empty() {
            args.push("--ignore-patterns".into());
            args.push(cli_arg.trim().to_owned());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        result = Some(<RulesConfig as ortho_config::OrthoConfig>::load_from_iter(
            refs,
        ));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    world.result = result;
    ensure!(
        world.result.is_some(),
        "configuration load did not produce a result"
    );
    Ok(())
}

#[then(expr = "the ignore patterns are {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn check_ignore(world: &mut World, expected_patterns: String) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result.map_err(|err| anyhow!(err))?;
    let want: Vec<String> = expected_patterns
        .split(',')
        .map(|s| s.trim().to_owned())
        .collect();
    ensure!(
        cfg.ignore_patterns == want,
        "unexpected ignore patterns {:?}; expected {:?}",
        cfg.ignore_patterns,
        want
    );
    Ok(())
}
