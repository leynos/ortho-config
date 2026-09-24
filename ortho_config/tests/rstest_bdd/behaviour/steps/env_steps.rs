//! Step definitions for environment variable testing.
//!
//! Provides BDD steps for setting environment variables, loading configuration
//! using [`CsvEnv`], and verifying parsed results.

use super::common::{SlotTakeOrExt, set_nonblank_scalar_once};
use super::value_parsing::{normalize_scalar, parse_csv_values};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{given, then, when};
use std::sync::Arc;

/// Records `DDLINT_RULES` for the injected test environment.
#[given("the environment variable DDLINT_RULES is {value}")]
fn set_env(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    set_nonblank_scalar_once(&rules_context.env_value, value, "environment rule value")
}

/// Loads configuration solely from the injected `DDLINT_RULES` environment value.
#[when("the configuration is loaded")]
fn load_config(rules_context: &RulesContext) -> Result<()> {
    let value = rules_context
        .env_value
        .get()
        .ok_or_else(|| anyhow!("environment value not configured"))?;
    let fixture_dir = tempfile::tempdir().context("create CSV selector fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open CSV selector fixture directory")?;
    fixture
        .write("selected.toml", b"# source-aware CSV selector fixture\n")
        .context("write CSV selector fixture")?;
    // A valid explicit selector prevents the generated loader reaching the
    // ambient current-working-directory fallback during this pure env scenario.
    let source = Arc::new(
        MapEnv::new()
            .with_var(
                "DDLINT_CONFIG_PATH",
                fixture_dir.path().join("selected.toml"),
            )
            .with_var("DDLINT_RULES", &value),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let config_result = RulesConfig::load_from_iter_with_sources(["prog"], discovery, merge);
    rules_context.result.set(config_result);
    Ok(())
}

/// Verifies that the parsed rule list matches the expected string.
#[then("the rules are {rules}")]
fn check_rules(rules_context: &RulesContext, rules: String) -> Result<()> {
    let result = rules_context
        .result
        .take_or("configuration result unavailable")?;
    let cfg = result.context("failed to parse rules configuration")?;
    let want = parse_csv_values(&rules);
    ensure!(
        cfg.rules == want,
        "unexpected rules {:?}; expected {:?}",
        cfg.rules,
        want
    );
    Ok(())
}
