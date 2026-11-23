//! Steps demonstrating a renamed configuration path flag.

use super::common::{SlotTakeOrExt, config_fixture, set_nonblank_scalar_once, shared_sources};
use super::value_parsing::{is_cli_parsing_error, normalize_scalar};
use crate::rstest_bdd_support::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig};
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsString;

/// The selector variable derived for `RulesConfig`'s legacy discovery.
const SELECTOR_VAR: &str = "DDLINT_CONFIG_PATH";
/// The project-local candidate name derived for `RulesConfig`.
const AUTOMATIC_FILE: &str = ".ddlint.toml";

#[given("an alternate config file with rule {value}")]
fn alt_config_file(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_nonblank_scalar_once(&rules_context.file_value, value, "alternate config rule")
}

#[given("a valid automatic configuration file with rule {value}")]
fn automatic_config_file(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_nonblank_scalar_once(&rules_context.auto_value, value, "automatic config rule")
}

#[given("the environment-selected configuration path does not exist")]
fn missing_selector(rules_context: &RulesContext) -> Result<()> {
    ensure!(
        rules_context.missing_selector.is_empty(),
        "missing selector already configured"
    );
    rules_context.missing_selector.set(());
    Ok(())
}

/// Write the automatic candidate and return its guard with a closed environment.
///
/// The working directory is pinned at the fixture, so the derived project root
/// is the only automatic location that can match. Home and XDG bases point at
/// absent subdirectories, keeping the scenario independent of the host.
fn automatic_fixture(rules_context: &RulesContext) -> Result<(tempfile::TempDir, MapEnv)> {
    let value = rules_context
        .auto_value
        .take_or("automatic config rule not provided")?;
    let (fixture_dir, _config_path) =
        config_fixture(AUTOMATIC_FILE, Some(&format!("rules = [\"{value}\"]")))?;
    let env = MapEnv::new()
        .with_current_dir(fixture_dir.path())
        .with_var("HOME", fixture_dir.path().join("no-home"))
        .with_var("XDG_CONFIG_HOME", fixture_dir.path().join("no-xdg-home"))
        .with_var("XDG_CONFIG_DIRS", fixture_dir.path().join("no-xdg-dirs"));
    Ok((fixture_dir, env))
}

#[when("the config is loaded with the injected environment")]
fn load_with_injected_environment(rules_context: &RulesContext) -> Result<()> {
    let (fixture_dir, mut env) = automatic_fixture(rules_context)?;
    if !rules_context.missing_selector.is_empty() {
        env = env.with_var(SELECTOR_VAR, fixture_dir.path().join("missing.toml"));
    }
    let (discovery, merge) = shared_sources(env);
    let config_result = RulesConfig::load_from_iter_with_sources(["prog"], discovery, merge);
    rules_context.result.set(config_result);
    Ok(())
}

#[when("the config is loaded with custom flag \"{flag}\" pointing at a missing file")]
fn load_with_missing_custom_flag(rules_context: &RulesContext, flag: String) -> Result<()> {
    let flag = normalize_scalar(&flag);
    let (fixture_dir, env) = automatic_fixture(rules_context)?;
    let missing = fixture_dir.path().join("missing.toml");
    let args = [
        OsString::from("prog"),
        OsString::from(flag),
        missing.into_os_string(),
    ];
    let (discovery, merge) = shared_sources(env);
    let config_result = RulesConfig::load_from_iter_with_sources(args, discovery, merge);
    rules_context.result.set(config_result);
    Ok(())
}

/// Returns true when `error` is, or contains, a file error.
fn is_file_error(error: &ortho_config::OrthoError) -> bool {
    match error {
        ortho_config::OrthoError::File { .. } => true,
        ortho_config::OrthoError::Aggregate(aggregated) => aggregated.iter().any(is_file_error),
        _ => false,
    }
}

#[then("config loading fails with a required file error")]
fn required_file_error(rules_context: &RulesContext) -> Result<()> {
    let result = rules_context
        .result
        .take_or("configuration result unavailable")?;
    match result {
        Ok(_) => Err(anyhow!(
            "expected a required file error but configuration succeeded"
        )),
        Err(err) => {
            if is_file_error(err.as_ref()) {
                Ok(())
            } else {
                Err(anyhow!("unexpected error: {err:?}"))
            }
        }
    }
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
