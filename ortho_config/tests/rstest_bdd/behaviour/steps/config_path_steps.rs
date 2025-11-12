//! Steps demonstrating a renamed configuration path flag.

use crate::fixtures::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};

#[given("an alternate config file with rule {value}")]
fn alt_config_file(rules_context: &RulesContext, value: String) -> Result<()> {
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
    let file_val = rules_context
        .file_value
        .take()
        .ok_or_else(|| anyhow!("alternate config file value not provided"))?;
    let mut result = None;
    figment::Jail::try_with(|j| {
        j.create_file(&path, &format!("rules = [\"{file_val}\"]"))?;
        let args = ["prog", flag.as_str(), path.as_str()];
        result = Some(RulesConfig::load_from_iter(args));
        Ok(())
    })
    .map_err(anyhow::Error::new)?;
    let config_result =
        result.ok_or_else(|| anyhow!("configuration load did not produce a result"))?;
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
            other => Err(anyhow!("unexpected error: {other:?}")),
        },
    }
}
