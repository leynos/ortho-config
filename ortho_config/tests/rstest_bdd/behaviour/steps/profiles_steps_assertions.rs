//! Assertion steps for profile selection and layering scenarios
//! (`profiles.feature`).
//!
//! The `then` steps read the structured result and error slots recorded by
//! [`profiles_steps`]' load steps, so one scenario can assert both the
//! selection and the unknown-profile error without consuming the result.

use crate::scenario_state::ProfilesContext;
use anyhow::{Result, anyhow, ensure};
use ortho_config::ProfileSource;
use rstest_bdd_macros::then;

use super::value_parsing::normalize_scalar;

/// Asserts the merged value of a key.
#[then("the merged value of {key} is {value}")]
fn merged_value_is(profiles_context: &ProfilesContext, key: String, value: String) -> Result<()> {
    let result = profiles_context
        .result
        .take()
        .ok_or_else(|| anyhow!("profile load result unavailable"))?;
    let config = result.map_err(anyhow::Error::from)?;
    let expected = normalize_scalar(&value);
    match normalize_scalar(&key).as_str() {
        "retries" => ensure!(
            config.retries.to_string() == expected,
            "unexpected retries {}; expected {expected}",
            config.retries
        ),
        other => {
            return Err(anyhow!(
                "unsupported key {other:?} in merged-value assertion"
            ));
        }
    }
    Ok(())
}

/// Asserts the winning selection and its source.
#[then("the selected profile is {profile} with source {source}")]
fn selected_profile_is(
    profiles_context: &ProfilesContext,
    profile: String,
    source: String,
) -> Result<()> {
    let selection = profiles_context
        .selection
        .take()
        .ok_or_else(|| anyhow!("selection unavailable"))?;
    let selected = selection
        .first()
        .ok_or_else(|| anyhow!("expected a selected profile, got none"))?;
    ensure!(
        selected.name.as_str() == normalize_scalar(&profile),
        "unexpected selected profile {:?}; expected {profile}",
        selected.name
    );
    ensure!(
        selected.source == parse_source(&source),
        "unexpected selection source {:?}; expected {source}",
        selected.source
    );
    Ok(())
}

/// Asserts a failed load names the profile and its flag selection source.
#[then("loading fails naming {profile} from source {source}")]
fn loading_fails_naming(
    profiles_context: &ProfilesContext,
    profile: String,
    source: String,
) -> Result<()> {
    let selected = profiles_context
        .error_selected
        .take()
        .ok_or_else(|| anyhow!("unknown-profile error not recorded"))?;
    let error_source = profiles_context
        .error_source
        .take()
        .ok_or_else(|| anyhow!("unknown-profile error source not recorded"))?;
    ensure!(
        selected == normalize_scalar(&profile),
        "unexpected unknown profile {selected:?}; expected {profile}"
    );
    ensure!(
        error_source == parse_source(&source),
        "unexpected selection source {error_source:?}; expected {source}"
    );
    Ok(())
}

/// Asserts a failed load names the profile and the selector environment source.
#[then("loading fails naming {profile} from the selector environment variable")]
fn loading_fails_naming_env(profiles_context: &ProfilesContext, profile: String) -> Result<()> {
    let selected = profiles_context
        .error_selected
        .take()
        .ok_or_else(|| anyhow!("unknown-profile error not recorded"))?;
    let error_source = profiles_context
        .error_source
        .take()
        .ok_or_else(|| anyhow!("unknown-profile error source not recorded"))?;
    ensure!(
        selected == normalize_scalar(&profile),
        "unexpected unknown profile {selected:?}; expected {profile}"
    );
    ensure!(
        error_source == ProfileSource::Environment,
        "expected the selector environment variable as the source, got {error_source:?}"
    );
    Ok(())
}

/// Asserts the unknown-profile error lists the available profiles.
#[then("the error lists available profiles {first} and {second}")]
fn error_lists_available(
    profiles_context: &ProfilesContext,
    first: String,
    second: String,
) -> Result<()> {
    let available = profiles_context
        .error_available
        .take()
        .ok_or_else(|| anyhow!("unknown-profile error not recorded"))?;
    let expected = vec![normalize_scalar(&first), normalize_scalar(&second)];
    ensure!(
        available == expected,
        "unexpected available profiles {available:?}; expected {expected:?}"
    );
    Ok(())
}

/// Asserts the load failed because no configuration files were found.
#[then("the error states that no configuration files were found")]
fn error_states_no_files(profiles_context: &ProfilesContext) -> Result<()> {
    let message = profiles_context
        .error_message
        .take()
        .ok_or_else(|| anyhow!("load error message not recorded"))?;
    ensure!(
        message.contains("no configuration files were found"),
        "error should state that no configuration files were found: {message}"
    );
    Ok(())
}

/// Asserts the load failed identifying the forbidden key in a profile.
#[then("loading fails identifying the forbidden {key} key in {profile}")]
fn loading_fails_forbidden_key(
    profiles_context: &ProfilesContext,
    key: String,
    profile: String,
) -> Result<()> {
    let offending = profiles_context
        .error_profile
        .take()
        .ok_or_else(|| anyhow!("forbidden-key error not recorded"))?;
    let offending_key = profiles_context
        .error_key
        .take()
        .ok_or_else(|| anyhow!("forbidden-key error not recorded"))?;
    ensure!(
        offending == normalize_scalar(&profile),
        "unexpected profile {offending:?}; expected {profile}"
    );
    ensure!(
        offending_key == normalize_scalar(&key),
        "unexpected key {offending_key:?}; expected {key}"
    );
    Ok(())
}
fn parse_source(source: &str) -> ProfileSource {
    match normalize_scalar(source).as_str() {
        "flag" => ProfileSource::Flag,
        _ => ProfileSource::Environment,
    }
}
