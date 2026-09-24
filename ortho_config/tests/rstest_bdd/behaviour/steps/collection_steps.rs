//! Steps covering collection merge strategy scenarios.

use super::common::{SlotTakeOrExt, set_scalar_once};
use crate::scenario_state::{CollectionContext, RulesConfig};
use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{given, then, when};
use std::sync::Arc;

#[given("the dynamic rules config enables {rule_name} via the configuration file")]
fn dynamic_rules_file(collection_context: &CollectionContext, rule_name: String) -> Result<()> {
    let section = format!("[dynamic_rules.{rule_name}]\nenabled = true\n");
    set_scalar_once(
        &collection_context.dynamic_rules_file,
        section,
        "dynamic rules file",
    )
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
    let fixture_dir = tempfile::tempdir().context("create collection config fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open collection config fixture directory")?;
    fixture
        .write(".ddlint.toml", file.as_deref().unwrap_or("").as_bytes())
        .context("write collection config fixture")?;
    let config_path = fixture_dir.path().join(".ddlint.toml");
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", config_path);
    for (name, enabled) in &env_rules {
        let normalised = name.replace('-', "_").to_ascii_uppercase();
        source = source.with_var(
            format!("DDLINT_DYNAMIC_RULES__{normalised}__ENABLED"),
            if *enabled { "true" } else { "false" },
        );
    }
    let source = Arc::new(source);
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let config_result = RulesConfig::load_from_iter_with_sources(["prog"], discovery, merge);
    collection_context.result.set(config_result);
    Ok(())
}

#[then("only the dynamic rule {rule_name} is enabled")]
fn assert_only_rule(collection_context: &CollectionContext, rule_name: String) -> Result<()> {
    let result = collection_context
        .result
        .take_or("configuration result unavailable")?;
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
