//! Steps demonstrating a renamed configuration path flag.

use super::value_parsing::normalize_scalar;
use crate::fixtures::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

#[given("an alternate config file with rule {value}")]
fn alt_config_file(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    ensure!(
        !value.trim().is_empty(),
        "alternate config rule must not be empty"
    );
    ensure!(
        rules_context.file_value.is_empty(),
        "alternate config file already initialised"
    );
    rules_context.file_value.set(value);
    Ok(())
}

#[when("the config is loaded with custom flag \"{flag}\" \"{path}\"")]
fn load_with_custom_flag(rules_context: &RulesContext, flag: String, path: String) -> Result<()> {
    let flag = normalize_scalar(&flag);
    let path = normalize_scalar(&path);
    let file_val = rules_context
        .file_value
        .take()
        .ok_or_else(|| anyhow!("alternate config file value not provided"))?;
    let config_result = figment_helpers::with_jail(|j| {
        j.create_file(&path, &format!("rules = [\"{file_val}\"]"))?;
        let args = ["prog", flag.as_str(), path.as_str()];
        Ok(RulesConfig::load_from_iter(args))
    })?;
    rules_context.result.set(config_result);
    Ok(())
}

#[then("config loading fails with a CLI parsing error")]
fn cli_error(rules_context: &RulesContext) -> Result<()> {
    let result = rules_context
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    match result {
        Ok(_) => Err(anyhow!(
            "expected CLI parsing error but configuration succeeded"
        )),
        Err(err) => match err.as_ref() {
            ortho_config::OrthoError::CliParsing(_) => Ok(()),
            ortho_config::OrthoError::Aggregate(agg)
                if agg
                    .iter()
                    .any(|entry| matches!(entry, ortho_config::OrthoError::CliParsing(_))) =>
            {
                Ok(())
            }
            other => Err(anyhow!("unexpected error: {other:?}")),
        },
    }
}
