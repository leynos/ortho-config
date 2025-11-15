//! Steps covering collection merge strategy scenarios.

use crate::fixtures::{CollectionContext, RulesConfig};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

#[given("the dynamic rules config enables {rule_name} via the configuration file")]
fn dynamic_rules_file(collection_context: &CollectionContext, rule_name: String) -> Result<()> {
    ensure!(
        collection_context.dynamic_rules_file.is_empty(),
        "dynamic rules file already initialised"
    );
    let section = format!("[dynamic_rules.{rule_name}]\nenabled = true\n");
    collection_context.dynamic_rules_file.set(section);
    Ok(())
}

#[given("the environment defines dynamic rule {rule_name} as {state}")]
fn dynamic_rule_env(
    collection_context: &CollectionContext,
    rule_name: String,
    state: String,
) -> Result<()> {
    let enabled = match state.as_str() {
        "enabled" => true,
        "disabled" => false,
        other => {
            return Err(anyhow!(
                "unexpected dynamic rule state '{other}'; expected 'enabled' or 'disabled'"
            ));
        }
    };
    let mut env_rules = collection_context
        .dynamic_rules_env
        .get_or_insert_with(Vec::new);
    env_rules.push((rule_name, enabled));
    Ok(())
}

#[when("the configuration is loaded with replace map semantics")]
fn load_replace_map(collection_context: &CollectionContext) -> Result<()> {
    let file = collection_context.dynamic_rules_file.take();
    let env_rules = collection_context
        .dynamic_rules_env
        .take()
        .unwrap_or_else(Vec::new);
    let config_result = figment_helpers::with_jail(|j| {
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
        Ok(RulesConfig::load_from_iter(["prog"]))
    })?;
    collection_context.result.set(config_result);
    Ok(())
}

#[then("only the dynamic rule {rule_name} is enabled")]
fn assert_only_rule(collection_context: &CollectionContext, rule_name: String) -> Result<()> {
    let result = collection_context
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
