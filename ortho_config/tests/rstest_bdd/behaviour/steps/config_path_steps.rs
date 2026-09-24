//! Steps demonstrating a renamed configuration path flag.

use super::common::{SlotTakeOrExt, set_nonblank_scalar_once};
use super::value_parsing::{is_cli_parsing_error, normalize_scalar};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Context as _, Result, anyhow};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{given, then, when};
use std::{ffi::OsString, sync::Arc};

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
    let fixture_dir = tempfile::tempdir().context("create custom-config fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open custom-config fixture directory")?;
    fixture
        .write(&path, format!("rules = [\"{file_val}\"]").as_bytes())
        .context("write custom-config fixture")?;
    let fixture_path = fixture_dir.path().join(&path);
    let source = Arc::new(MapEnv::new());
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
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
