//! Steps that validate the merge composer builder output.

use super::value_parsing::{normalize_scalar, parse_csv_values};
use crate::scenario_state::{ComposerContext, RulesConfig, RulesContext};
use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, MergeProvenance, OrthoError, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{then, when};
use std::sync::Arc;

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
    let fixture_dir = tempfile::tempdir().context("create layer-composition fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open layer-composition fixture directory")?;
    if let Some(value) = file_val.as_ref() {
        fixture
            .write(".ddlint.toml", format!("rules = [\"{value}\"]").as_bytes())
            .context("write layer-composition configuration fixture")?;
    }
    let config_path = fixture_dir.path().join(".ddlint.toml");
    let mut source = MapEnv::new().with_var("DDLINT_CONFIG_PATH", &config_path);
    if let Some(value) = env_val.as_ref() {
        source = source.with_var("DDLINT_RULES", value);
    }
    let source = Arc::new(source);
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let composition = RulesConfig::compose_layers_from_iter_with_sources(
        [binary_name, "--rules", cli_rules.as_str()],
        discovery,
        merge,
    );

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
