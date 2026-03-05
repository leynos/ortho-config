//! Steps for testing ignore pattern list handling.

use super::value_parsing::{normalize_scalar, parse_csv_values};
use crate::scenario_state::{RulesConfig, RulesContext};
use anyhow::{Result, anyhow, ensure};
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

#[given("the environment variable DDLINT_IGNORE_PATTERNS is {value}")]
fn set_ignore_env(rules_context: &RulesContext, value: String) -> Result<()> {
    let value = normalize_scalar(&value);
    ensure!(
        rules_context.env_value.is_empty(),
        "ignore patterns environment value already initialised"
    );
    rules_context.env_value.set(value);
    Ok(())
}

#[when("the config is loaded with CLI ignore {cli}")]
fn load_ignore(rules_context: &RulesContext, cli: String) -> Result<()> {
    let cli = normalize_scalar(&cli);
    let env_val = rules_context.env_value.take();
    let config_result = figment_helpers::with_jail(|j| {
        if let Some(val) = env_val
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            j.set_env("DDLINT_IGNORE_PATTERNS", val);
        }
        let mut args = vec![String::from("prog")];
        if !cli.is_empty() {
            args.push("--ignore-patterns".into());
            args.push(cli.trim().to_owned());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        Ok(<RulesConfig as ortho_config::OrthoConfig>::load_from_iter(
            refs,
        ))
    })?;
    rules_context.result.set(config_result);
    Ok(())
}

#[then("the ignore patterns are {patterns}")]
fn check_ignore(rules_context: &RulesContext, patterns: String) -> Result<()> {
    let result = rules_context
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    let cfg = result.map_err(anyhow::Error::from)?;
    let want = parse_csv_values(&patterns);
    ensure!(
        cfg.ignore_patterns == want,
        "unexpected ignore patterns {:?}; expected {:?}",
        cfg.ignore_patterns,
        want
    );
    Ok(())
}
