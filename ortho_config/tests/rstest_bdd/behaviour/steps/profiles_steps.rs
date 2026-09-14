//! Steps for profile selection and layering scenarios (`profiles.feature`).
//!
//! The scenarios exercise the generated entry point
//! `ProfilesConfig::load_with_profile_from_iter` against a jailed config file
//! and environment: the `--profile` flag, the `APP_PROFILE` selector, the
//! profile merge layer, and the flag-equals-default fix all run through the
//! derived CLI. Assertion steps live in the sibling
//! `profiles_steps_assertions` module.

use crate::scenario_state::{ProfilesConfig, ProfilesContext};
use anyhow::{Result, ensure};
use ortho_config::{OrthoError, OrthoResult};
use rstest_bdd_macros::{given, when};
use serde_json::{Map, Value};

use super::value_parsing::{normalize_scalar, unquote};

/// Records a base file key for the profile scenario's configuration file.
#[given("a config file with key {key} set to {value}")]
fn config_file_key(profiles_context: &ProfilesContext, key: String, value: String) -> Result<()> {
    profiles_context
        .base_keys
        .get_or_insert_with(Vec::new)
        .push((normalize_scalar(&key), normalize_scalar(&value)));
    Ok(())
}

/// Records one normalized profile key for the active scenario.
fn record_profile_key(profiles_context: &ProfilesContext, profile: &str, key: &str, value: &str) {
    profiles_context
        .profile_keys
        .get_or_insert_with(Vec::new)
        .push((
            normalize_scalar(profile),
            normalize_scalar(key),
            normalize_scalar(value),
        ));
}

/// Records a profile table key for the same configuration file.
#[given("the same file defines profile {profile} with {key} set to {value}")]
fn same_file_profile_key(
    profiles_context: &ProfilesContext,
    profile: String,
    key: String,
    value: String,
) -> Result<()> {
    record_profile_key(profiles_context, &profile, &key, &value);
    Ok(())
}

/// Records a profile table key for a freshly described configuration file.
#[given("a config file defining profile {profile} with {key} set to {value}")]
fn config_file_profile_key(
    profiles_context: &ProfilesContext,
    profile: String,
    key: String,
    value: String,
) -> Result<()> {
    same_file_profile_key(profiles_context, profile, key, value)
}

/// Records empty profile tables for the named profiles.
#[given("a config file defining profiles {first} and {second}")]
fn config_file_profiles(
    profiles_context: &ProfilesContext,
    first: String,
    second: String,
) -> Result<()> {
    for profile in [first, second] {
        record_profile_key(profiles_context, &profile, "", "");
    }
    Ok(())
}

/// Records a profile table containing a forbidden key (for example `cmds`).
#[given("a config file defining profile {profile} containing a {key} table")]
fn config_file_forbidden_key(
    profiles_context: &ProfilesContext,
    profile: String,
    key: String,
) -> Result<()> {
    record_profile_key(profiles_context, &profile, &key, "__table__");
    Ok(())
}

/// Records the struct default for the flag-equals-default scenario.
///
/// `ProfilesConfig` bakes its `retries` default in at compile time, so the
/// recorded value must match it for the scenario to be meaningful.
#[given("a struct default of {value} for {key}")]
fn struct_default(profiles_context: &ProfilesContext, value: String, key: String) -> Result<()> {
    ensure!(
        !normalize_scalar(&key).is_empty(),
        "struct default key must not be empty"
    );
    ensure!(
        profiles_context.struct_default.is_empty(),
        "struct default already initialised"
    );
    ensure!(
        normalize_scalar(&value) == "3",
        "ProfilesConfig retries default is baked in at compile time; expected 3, got {value:?}"
    );
    profiles_context.struct_default.set(value);
    Ok(())
}

/// Marks the scenario as having no discoverable configuration files.
#[given("no configuration files are discoverable")]
fn no_config_files(profiles_context: &ProfilesContext) -> Result<()> {
    profiles_context.no_files.set(());
    Ok(())
}

/// Records the `APP_PROFILE` selector environment value.
#[given("the selector environment variable names profile {profile}")]
fn selector_env(profiles_context: &ProfilesContext, profile: String) -> Result<()> {
    profiles_context
        .selector_env
        .set(normalize_scalar(&profile));
    Ok(())
}

/// Records an `APP_`-prefixed environment override.
#[given("the environment sets the {key} key to {value}")]
fn env_key(profiles_context: &ProfilesContext, key: String, value: String) -> Result<()> {
    profiles_context
        .env_keys
        .get_or_insert_with(Vec::new)
        .push((normalize_scalar(&key), normalize_scalar(&value)));
    Ok(())
}

/// Runs the profile-aware load with the given flags.
#[when("the CLI loads with {flags}")]
fn profiles_load_with_flags(profiles_context: &ProfilesContext, flags: String) -> Result<()> {
    let parsed = parse_flags(&normalize_scalar(&flags));
    profiles_context
        .result
        .set(profile_load(profiles_context, &parsed));
    Ok(())
}

/// Runs the profile-aware load with no flags.
#[when("the CLI loads")]
fn profiles_load(profiles_context: &ProfilesContext) -> Result<()> {
    profiles_context
        .result
        .set(profile_load(profiles_context, &[]));
    Ok(())
}
/// Records the structured fields of a load error for later assertions.
fn record_load_error(profiles_context: &ProfilesContext, err: &OrthoError) {
    profiles_context.error_message.set(err.to_string());
    match err {
        OrthoError::UnknownProfile {
            selected,
            selection_source,
            available,
        } => {
            profiles_context.error_selected.set(selected.clone());
            profiles_context.error_source.set(*selection_source);
            profiles_context
                .error_available
                .set(available.as_slice().to_vec());
        }
        OrthoError::ProfileForbiddenKey { profile, key } => {
            profiles_context.error_profile.set(profile.clone());
            profiles_context.error_key.set(key.clone());
        }
        _ => {}
    }
}

/// Runs the profile-aware load through the generated entry point.
///
/// Writes the scenario's config file and environment into a jail, then calls
/// `ProfilesConfig::load_with_profile_from_iter` so the scenarios exercise the
/// real derived CLI: the `--profile` flag, the `APP_PROFILE` selector, the
/// profile merge layer, and the flag-equals-default fix.
fn profile_load(
    profiles_context: &ProfilesContext,
    flags: &[(String, String)],
) -> OrthoResult<ortho_config::ProfileLoadOutcome<ProfilesConfig>> {
    let args = build_cli_args(flags);
    let selector = profiles_context.selector_env.get();
    let env_keys = profiles_context.env_keys.get().unwrap_or_default();
    let file_value = if profiles_context.no_files.is_empty() {
        Some(build_file_value(profiles_context))
    } else {
        None
    };

    let result = test_helpers::figment::with_jail(|j| {
        if let Some(value) = selector.as_ref() {
            j.set_env("APP_PROFILE", value);
        }
        for (key, value) in &env_keys {
            j.set_env(format!("APP_{}", key.to_ascii_uppercase()), value);
        }
        if let Some(value) = file_value.as_ref() {
            let content = ortho_config::toml::to_string(value)
                .map_err(|err| figment::error::Error::from(err.to_string()))?;
            j.create_file(".app.toml", &content)?;
        }
        let composition = ProfilesConfig::compose_layers_from_iter(args.clone());
        profiles_context.layers.set(composition.into_parts().0);
        Ok(ProfilesConfig::load_with_profile_from_iter(args.clone()))
    })
    .map_err(|err| {
        std::sync::Arc::new(OrthoError::Validation {
            key: "jail".to_owned(),
            message: format!("profile scenario jail setup failed: {err}"),
        })
    })?;

    match &result {
        Ok(outcome) => {
            profiles_context.selection.set(outcome.selection().to_vec());
        }
        Err(err) => record_load_error(profiles_context, err),
    }
    result
}

/// Builds the CLI argument vector from the parsed `--flag value` pairs.
fn build_cli_args(flags: &[(String, String)]) -> Vec<String> {
    let mut args = vec!["profile-cli".to_owned()];
    for (flag, value) in flags {
        args.push(format!("--{flag}"));
        args.push(value.clone());
    }
    args
}

/// Builds the file value from the accumulated base keys and profile tables.
fn build_file_value(profiles_context: &ProfilesContext) -> Value {
    let mut file = Map::new();
    let base_keys = profiles_context.base_keys.get().unwrap_or_default();
    for (key, value) in base_keys {
        file.insert(key, scalar_value(&value));
    }
    let profile_keys = profiles_context.profile_keys.get().unwrap_or_default();
    if !profile_keys.is_empty() {
        let mut profiles = Map::new();
        for (profile, key, value) in profile_keys {
            let table = profiles
                .entry(profile)
                .or_insert_with(|| Value::Object(Map::new()));
            let Value::Object(table_map) = table else {
                continue;
            };
            if key.is_empty() {
                continue;
            }
            if value == "__table__" {
                table_map.insert(key, Value::Object(Map::new()));
            } else {
                table_map.insert(key, scalar_value(&value));
            }
        }
        file.insert("profile".to_owned(), Value::Object(profiles));
    }
    Value::Object(file)
}

/// Parses `--flag value` pairs from a whitespace-separated flag string.
fn parse_flags(flags: &str) -> Vec<(String, String)> {
    let tokens: Vec<&str> = flags.split_whitespace().collect();
    tokens
        .chunks(2)
        .filter_map(|pair| {
            let flag = pair.first()?.trim_start_matches("--").to_owned();
            let value = (*pair.get(1)?).to_owned();
            Some((flag, value))
        })
        .collect()
}

/// Converts a scalar placeholder to a JSON value, preserving numbers.
fn scalar_value(value: &str) -> Value {
    let value = unquote(value);
    if let Ok(number) = value.parse::<u64>() {
        return Value::from(number);
    }
    Value::String(value.to_owned())
}
