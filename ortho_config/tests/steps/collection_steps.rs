//! Steps covering collection merge strategy scenarios.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{RulesConfig, World};
use anyhow::{Result, anyhow, ensure};
use cucumber::{given, then, when};
use ortho_config::OrthoConfig;

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber captures step parameters as owned Strings"
)]
#[given(expr = "the dynamic rules config enables {string} via the configuration file")]
fn dynamic_rules_file(world: &mut World, name: String) -> Result<()> {
    ensure!(
        world.dynamic_rules_file.is_none(),
        "dynamic rules file already initialised",
    );
    let section = format!("[dynamic_rules.{name}]\nenabled = true\n");
    world.dynamic_rules_file = Some(section);
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber captures step parameters as owned Strings"
)]
#[given(expr = "the environment defines dynamic rule {string} as {word}")]
fn dynamic_rule_env(world: &mut World, name: String, state: String) -> Result<()> {
    let enabled = match state.as_str() {
        "enabled" => true,
        "disabled" => false,
        other => {
            return Err(anyhow!(
                "unexpected dynamic rule state '{other}'; expected 'enabled' or 'disabled'"
            ));
        }
    };
    world.dynamic_rules_env.push((name, enabled));
    Ok(())
}

#[when("the configuration is loaded with replace map semantics")]
fn load_replace_map(world: &mut World) -> Result<()> {
    let file = std::mem::take(&mut world.dynamic_rules_file);
    let env_rules = std::mem::take(&mut world.dynamic_rules_env);
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
    })?;
    world.result = result;
    ensure!(
        world.result.is_some(),
        "configuration load did not produce a result",
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber captures step parameters as owned Strings"
)]
#[then(expr = "only the dynamic rule {string} is enabled")]
fn assert_only_rule(world: &mut World, name: String) -> Result<()> {
    let result = world
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result?;
    ensure!(
        cfg.dynamic_rules.len() == 1,
        "expected exactly one dynamic rule, found {:?}",
        cfg.dynamic_rules,
    );
    let rule = cfg
        .dynamic_rules
        .get(&name)
        .ok_or_else(|| anyhow!("expected dynamic rule {name:?}"))?;
    ensure!(rule.enabled, "dynamic rule {name:?} must be enabled");
    Ok(())
}
