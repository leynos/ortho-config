//! Steps that validate the merge composer builder output.

use crate::fixtures::{ComposerContext, RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MergeProvenance, OrthoConfig, OrthoError};
use rstest_bdd_macros::{then, when};
use test_helpers::figment as figment_helpers;

#[when("the rule layers are composed with CLI rules {cli_rules}")]
fn compose_rule_layers(
    rules_context: &RulesContext,
    composer_context: &ComposerContext,
    binary_name: &str,
    cli_rules: String,
) -> Result<()> {
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

    composer_context.layers.set(layers.clone());
    let config = RulesConfig::merge_from_layers(layers).map_err(|err| anyhow!(err))?;
    composer_context.config.set(config);
    Ok(())
}

#[then("the composed layer order is defaults, file, environment, cli")]
fn composed_order_is_stable(composer_context: &ComposerContext) -> Result<()> {
    let layers = composer_context
        .layers
        .with_ref(|layers| layers.clone())
        .ok_or_else(|| anyhow!("expected layers to be composed"))?;
    let provenances: Vec<MergeProvenance> =
        layers.iter().map(|layer| layer.provenance()).collect();
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
    let config = composer_context
        .config
        .with_ref(|cfg| cfg.clone())
        .ok_or_else(|| anyhow!("expected configuration to be composed"))?;
    let expected_rules: Vec<String> =
        expected.split(',').map(|value| value.trim().to_owned()).collect();
    ensure!(
        config.rules == expected_rules,
        "unexpected rules {:?}; expected {:?}",
        config.rules,
        expected_rules
    );
    Ok(())
}
