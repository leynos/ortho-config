//! Steps covering collection merge strategy scenarios.

use crate::fixtures::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};

#[given("the dynamic rules config enables {rule_name} via the configuration file")]
fn dynamic_rules_file(world: &World, rule_name: String) -> Result<()> {
    ensure!(
        world.dynamic_rules_file.is_empty(),
        "dynamic rules file already initialised"
    );
    let section = format!("[dynamic_rules.{rule_name}]\nenabled = true\n");
    world.dynamic_rules_file.set(section);
    Ok(())
}

#[given("the environment defines dynamic rule {rule_name} as {state}")]
fn dynamic_rule_env(world: &World, rule_name: String, state: String) -> Result<()> {
    let enabled = match state.as_str() {
        "enabled" => true,
        "disabled" => false,
        other => {
            return Err(anyhow!(
                "unexpected dynamic rule state '{other}'; expected 'enabled' or 'disabled'"
            ));
        }
    };
    let mut env_rules = world
        .dynamic_rules_env
        .get_or_insert_with(Vec::new);
    env_rules.push((rule_name, enabled));
    Ok(())
}

#[when("the configuration is loaded with replace map semantics")]
fn load_replace_map(world: &World) -> Result<()> {
    let file = world.dynamic_rules_file.take();
    let env_rules = world
        .dynamic_rules_env
        .take()
        .unwrap_or_else(Vec::new);
    let mut result = None;
    figment::Jail::try_with(|j| {
        if let Some(contents) = file.as_ref() {
            j.create_file(".ddlint.toml", contents)?;
        }
        for (name, enabled) in &env_rules {
            let normalised = name.replace('-', "_").to_ascii_uppercase();
            j.set_env(
                format!("DDLINT_DYNAMIC_RULES__{normalised}__ENABLED"),
                if *enabled { "true" } else { "false" },
            );
        }
        result = Some(RulesConfig::load_from_iter(["prog"]));
        Ok(())
    })
    .map_err(anyhow::Error::new)?;
    let config_result =
        result.ok_or_else(|| anyhow!("configuration load did not produce a result"))?;
    world.result.set(config_result);
    Ok(())
}

#[then("only the dynamic rule {rule_name} is enabled")]
fn assert_only_rule(world: &World, rule_name: String) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result?;
    ensure!(
        cfg.dynamic_rules.len() == 1,
        "expected exactly one dynamic rule, found {:?}",
        cfg.dynamic_rules
    );
    let rule = cfg
        .dynamic_rules
        .get(&rule_name)
        .ok_or_else(|| anyhow!("expected dynamic rule {rule_name:?}"))?;
    ensure!(rule.enabled, "dynamic rule {rule_name:?} must be enabled");
    Ok(())
}
