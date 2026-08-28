//! Steps demonstrating a renamed configuration path flag.

use super::common::{SlotTakeOrExt, set_nonblank_scalar_once};
use super::value_parsing::{is_cli_parsing_error, normalize_scalar};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow};
use ortho_config::OrthoConfig;
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

#[given("an alternate config file with rule {value}")]
fn alt_config_file(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_nonblank_scalar_once(&rules_context.file_value, value, "alternate config rule")
}

#[when("the config is loaded with custom flag \"{flag}\" \"{path}\"")]
fn load_with_custom_flag(rules_context: &RulesContext, flag: String, path: String) -> Result<()> {
    let flag = normalize_scalar(&flag);
    let path = normalize_scalar(&path);
    let file_val = rules_context
        .file_value
        .take_or("alternate config file value not provided")?;
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
        .take_or("configuration result unavailable")?;
    match result {
        Ok(_) => Err(anyhow!(
            "expected CLI parsing error but configuration succeeded"
        )),
        Err(err) => {
            if is_cli_parsing_error(err.as_ref()) {
                Ok(())
            } else {
                Err(anyhow!("unexpected error: {err:?}"))
            }
        }
    }
}
