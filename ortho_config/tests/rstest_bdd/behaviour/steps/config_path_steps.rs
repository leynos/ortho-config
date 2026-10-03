//! Steps demonstrating a renamed configuration path flag.

use super::common::{SlotTakeOrExt, config_fixture, set_nonblank_scalar_once, shared_sources};
use super::value_parsing::{is_cli_parsing_error, normalize_scalar};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow};
use ortho_config::{MapEnv, OrthoConfig};
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsString;

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
    // The guard stays bound until the loader below has read the fixture.
    let (_fixture_dir, fixture_path) =
        config_fixture(&path, Some(&format!("rules = [\"{file_val}\"]")))?;
    let (discovery, merge) = shared_sources(MapEnv::new());
    // The generated `--config` path is required, so this absolute fixture is
    // attempted before optional selectors and ambient discovery candidates.
    let args = [
        OsString::from("prog"),
        OsString::from(flag),
        fixture_path.into_os_string(),
    ];
    let config_result = RulesConfig::load_from_iter_with_sources(args, discovery, merge);
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
