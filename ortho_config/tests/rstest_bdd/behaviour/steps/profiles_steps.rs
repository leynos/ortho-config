//! Steps for profile selection and layering scenarios (`profiles.feature`).
//!
//! The scenarios drive the library profile helpers directly — resolution,
//! extraction, and merge — because the derived `--profile` flag arrives with
//! the opt-in derive in milestone 4. The `APP_` prefix gives the `APP_PROFILE`
//! selector and `APP_RETRIES` environment key. Assertion steps live in the
//! sibling `profiles_steps_assertions` module.

use crate::scenario_state::{ProfilesConfig, ProfilesContext};
use anyhow::{Result, ensure};
use ortho_config::{
    MergeComposer, OrthoError, OrthoResult, SelectedProfile, profile::extract_profile_layers,
};
use rstest_bdd_macros::{given, when};
use serde_json::{Map, Value, json};
use std::borrow::Cow;

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

/// Records a profile table key for the same configuration file.
#[given("the same file defines profile {profile} with {key} set to {value}")]
fn same_file_profile_key(
    profiles_context: &ProfilesContext,
    profile: String,
    key: String,
    value: String,
) -> Result<()> {
    profiles_context
        .profile_keys
        .get_or_insert_with(Vec::new)
        .push((
            normalize_scalar(&profile),
            normalize_scalar(&key),
            normalize_scalar(&value),
        ));
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
        profiles_context
            .profile_keys
            .get_or_insert_with(Vec::new)
            .push((normalize_scalar(&profile), String::new(), String::new()));
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
    profiles_context
        .profile_keys
        .get_or_insert_with(Vec::new)
        .push((
            normalize_scalar(&profile),
            normalize_scalar(&key),
            "__table__".to_owned(),
        ));
    Ok(())
}

/// Records the struct default for the flag-equals-default scenario.
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

/// Runs the profile-aware load: resolve the selection, extract the profile
/// tables, and merge into a `ProfilesConfig`, recording the selection for
/// later assertions.
fn profile_load(
    profiles_context: &ProfilesContext,
    flags: &[(String, String)],
) -> OrthoResult<ProfilesConfig> {
    let flag_profile = flag_value(flags, "profile");
    let env_profile = profiles_context.selector_env.get();
    let selection = SelectedProfile::resolve(flag_profile, env_profile.as_deref())?;
    profiles_context
        .selection
        .set(selection.clone().into_iter().collect());

    let file_layers: Vec<ortho_config::MergeLayer<'static>> =
        if profiles_context.no_files.is_empty() {
            vec![ortho_config::MergeLayer::file(
                Cow::Owned(build_file_value(profiles_context)),
                None,
            )]
        } else {
            Vec::new()
        };

    let outcome = match extract_profile_layers(file_layers, selection.as_ref()) {
        Ok(outcome) => outcome,
        Err(err) => {
            record_load_error(profiles_context, &err);
            return Err(err);
        }
    };

    let mut composer = MergeComposer::new();
    let defaults = profiles_context.struct_default.get();
    match normalize_scalar(defaults.as_deref().unwrap_or("")).as_str() {
        "" => composer.push_defaults(json!({})),
        value => composer.push_defaults(json!({ "retries": parse_u32(value)? })),
    }
    for layer in outcome.file_layers {
        composer.push_layer(layer);
    }
    for layer in outcome.profile_layers {
        composer.push_layer(layer);
    }

    let env_keys = profiles_context.env_keys.get().unwrap_or_default();
    if let Some((_, retries)) = env_keys.iter().find(|(key, _)| key == "retries") {
        composer.push_environment(json!({ "retries": parse_u32(retries)? }));
    }
    if let Some(retries) = flag_value(flags, "retries") {
        composer.push_cli(json!({ "retries": parse_u32(retries)? }));
    }

    ProfilesConfig::merge_from_layers(composer.layers())
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

/// Returns the value for `flag` from a parsed flag list, if present.
fn flag_value<'a>(flags: &'a [(String, String)], flag: &str) -> Option<&'a str> {
    flags
        .iter()
        .find(|(name, _)| name == flag)
        .map(|(_, value)| value.as_str())
}

/// Converts a scalar placeholder to a JSON value, preserving numbers.
fn scalar_value(value: &str) -> Value {
    let value = unquote(value);
    if let Ok(number) = value.parse::<u64>() {
        return Value::from(number);
    }
    Value::String(value.to_owned())
}

/// Parses a numeric placeholder as `u32`, reporting a validation error.
fn parse_u32(value: &str) -> OrthoResult<u32> {
    unquote(value).parse::<u32>().map_err(|_| {
        std::sync::Arc::new(OrthoError::Validation {
            key: "retries".to_owned(),
            message: format!("invalid retries value {value:?}"),
        })
    })
}
