//! Steps for testing ignore pattern list handling.

use crate::fixtures::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};

#[given("the environment variable DDLINT_IGNORE_PATTERNS is {value}")]
fn set_ignore_env(world: &World, value: String) -> Result<()> {
    ensure!(
        world.env_value.is_empty(),
        "ignore patterns environment value already initialised"
    );
    world.env_value.set(value);
    Ok(())
}

#[when("the config is loaded with CLI ignore {cli}")]
fn load_ignore(world: &World, cli: String) -> Result<()> {
    let env_val = world.env_value.take();
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(val) = env_val.as_deref() {
            j.set_env("DDLINT_IGNORE_PATTERNS", val);
        }
        let mut args = vec![String::from("prog")];
        if !cli.is_empty() {
            args.push("--ignore-patterns".into());
            args.push(cli.trim().to_owned());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        result = Some(<RulesConfig as ortho_config::OrthoConfig>::load_from_iter(
            refs,
        ));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    let config_result =
        result.ok_or_else(|| anyhow!("configuration load did not produce a result"))?;
    world.result.set(config_result);
    Ok(())
}

#[then("the ignore patterns are {patterns}")]
fn check_ignore(world: &World, patterns: String) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result.map_err(|err| anyhow!(err))?;
    let want: Vec<String> = patterns
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
