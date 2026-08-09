//! Steps that validate the merge composer builder output.

use super::value_parsing::{normalize_scalar, parse_csv_values};
use crate::scenario_state::{ComposerContext, RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MergeComposer, MergeLayer, MergeProvenance, OrthoError};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;
use test_helpers::figment as figment_helpers;

/// Records the value supplied by the profile layer for composer scenarios.
#[given("the profile layer has rules {value}")]
fn profile_layer_rules(composer_context: &ComposerContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    ensure!(
        !value.trim().is_empty(),
        "profile rule value must not be empty"
    );
    ensure!(
        composer_context.profile_value.is_empty(),
        "profile rule value already initialised"
    );
    composer_context.profile_value.set(value);
    Ok(())
}

#[when("the rule layers are composed with CLI rules {cli_rules}")]
fn compose_rule_layers(
    rules_context: &RulesContext,
    composer_context: &ComposerContext,
    binary_name: &str,
    cli_rules: String,
) -> Result<()> {
    let cli_rules = normalize_scalar(&cli_rules);
    let file_val = rules_context.file_value.get();
    let env_val = rules_context.env_value.get();
    let composition = figment_helpers::with_jail(|j| {
        if let Some(value) = file_val.as_ref() {
            j.create_file(".ddlint.toml", &format!("rules = [\"{value}\"]"))?;
        }
        if let Some(value) = env_val.as_ref() {
            j.set_env("DDLINT_RULES", value);
        }
        Ok(RulesConfig::compose_layers_from_iter([
            binary_name,
            "--rules",
            cli_rules.as_str(),
        ]))
    })?;

    let (layers, errors) = composition.into_parts();
    if let Some(err) = OrthoError::try_aggregate(errors) {
        return Err(anyhow!(err));
    }

    composer_context.layers.set(layers);
    let layers_for_merge = composer_context
        .layers
        .with_ref(|ls| ls.clone())
        .ok_or_else(|| anyhow!("layers should be available for merge"))?;
    let config = RulesConfig::merge_from_layers(layers_for_merge).map_err(anyhow::Error::from)?;
    composer_context.config.set(config);
    Ok(())
}

#[when("the rule layers are composed with the profile layer and CLI rules {cli_rules}")]
fn compose_rule_layers_with_profile(
    rules_context: &RulesContext,
    composer_context: &ComposerContext,
    cli_rules: String,
) -> Result<()> {
    let cli_rules = parse_csv_values(&normalize_scalar(&cli_rules));
    let file_val = rules_context.file_value.get();
    let env_val = rules_context.env_value.get();
    let profile_val = composer_context.profile_value.get();

    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({}));
    if let Some(value) = file_val.as_ref() {
        composer.push_file(json!({ "rules": [value] }), None);
    }
    if let Some(value) = profile_val.as_ref() {
        composer.push_profile(json!({ "rules": [value] }), None);
    }
    if let Some(value) = env_val.as_ref() {
        composer.push_environment(json!({ "rules": parse_csv_values(value) }));
    }
    composer.push_cli(json!({ "rules": cli_rules }));

    let layers = composer.layers();
    composer_context.layers.set(layers.clone());
    let config = RulesConfig::merge_from_layers(layers).map_err(anyhow::Error::from)?;
    composer_context.config.set(config);
    Ok(())
}

#[then("the composed layer order is defaults, file, profile, environment, cli")]
fn composed_order_with_profile(composer_context: &ComposerContext) -> Result<()> {
    let layers = composer_context
        .layers
        .with_ref(|layers| layers.clone())
        .ok_or_else(|| anyhow!("expected layers to be composed"))?;
    let provenances: Vec<MergeProvenance> = layers.iter().map(MergeLayer::provenance).collect();
    ensure!(
        provenances
            == vec![
                MergeProvenance::Defaults,
                MergeProvenance::File,
                MergeProvenance::Profile,
                MergeProvenance::Environment,
                MergeProvenance::Cli,
            ],
        "unexpected provenance ordering: {:?}",
        provenances
    );
    Ok(())
}

#[then("the composed layer order is defaults, file, environment, cli")]
fn composed_order_is_stable(composer_context: &ComposerContext) -> Result<()> {
    let layers = composer_context
        .layers
        .with_ref(|layers| layers.clone())
        .ok_or_else(|| anyhow!("expected layers to be composed"))?;
    let provenances: Vec<MergeProvenance> = layers.iter().map(|layer| layer.provenance()).collect();
    ensure!(
        provenances
            == vec![
                MergeProvenance::Defaults,
                MergeProvenance::File,
                MergeProvenance::Environment,
                MergeProvenance::Cli,
            ],
        "unexpected provenance ordering: {:?}",
        provenances
    );
    Ok(())
}

#[then("the merged rules resolve to {expected}")]
fn merged_rules_match(composer_context: &ComposerContext, expected: String) -> Result<()> {
    let rules = composer_context
        .config
        .with_ref(|cfg| cfg.rules.clone())
        .ok_or_else(|| anyhow!("expected configuration to be composed"))?;
    let expected_rules = parse_csv_values(&expected);
    ensure!(
        rules == expected_rules,
        "unexpected rules {:?}; expected {:?}",
        rules,
        expected_rules
    );
    Ok(())
}
