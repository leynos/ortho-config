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
/// Names the `OrthoError` variant carried by `err`.
///
/// The name identifies the variant without rendering its payload, so a
/// scenario can assert which errors a load retained against the precedence
/// contract rather than against message text.
fn variant_name(err: &OrthoError) -> &'static str {
    match err {
        OrthoError::CliParsing(_) => "CliParsing",
        OrthoError::UnknownProfile { .. } => "UnknownProfile",
        OrthoError::ProfileForbiddenKey { .. } => "ProfileForbiddenKey",
        OrthoError::InvalidProfileName { .. } => "InvalidProfileName",
        OrthoError::ReservedProfileName { .. } => "ReservedProfileName",
        OrthoError::Aggregate(_) => "Aggregate",
        _ => "Other",
    }
}

/// Flattens a load error into the errors a caller actually sees.
///
/// An aggregated failure reports every sub-error it retained; any other
/// failure reports itself. A scenario asserting on one sub-error must look
/// through the aggregate, since combining a parse error with a selection error
/// is exactly the case the precedence contract describes.
fn flatten_load_error(err: &OrthoError) -> Vec<&OrthoError> {
    match err {
        OrthoError::Aggregate(aggregated) => aggregated.iter().collect(),
        single => vec![single],
    }
}

/// Records the structured fields of a load error for later assertions.
fn record_load_error(profiles_context: &ProfilesContext, err: &OrthoError) {
    profiles_context.error_message.set(err.to_string());
    let reported = flatten_load_error(err);
    profiles_context.error_variants.set(
        reported
            .iter()
            .map(|inner| variant_name(inner).to_owned())
            .collect(),
    );
    for inner in reported {
        match inner {
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
///
/// An empty value marks a valueless flag, so only the flag itself is pushed.
/// The malformed-flag scenario needs clap to see `--bogus` with nothing after
/// it, which is what makes the argument unrecognised.
fn build_cli_args(flags: &[(String, String)]) -> Vec<String> {
    let mut args = vec!["profile-cli".to_owned()];
    for (flag, value) in flags {
        args.push(format!("--{flag}"));
        if !value.is_empty() {
            args.push(value.clone());
        }
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

/// Parses a whitespace-separated flag string into `(name, value)` pairs.
///
/// A token beginning with `--` starts a pair; the token after it is its value
/// only when that token is not itself flag-like. A flag with no value — the
/// malformed-flag scenario's `--bogus` — is therefore still emitted, with an
/// empty value, rather than being silently dropped by a fixed-stride grouping.
fn parse_flags(flags: &str) -> Vec<(String, String)> {
    let mut parsed: Vec<(String, String)> = Vec::new();
    for token in flags.split_whitespace() {
        match token.strip_prefix("--") {
            Some(name) => parsed.push((name.to_owned(), String::new())),
            None => {
                if let Some(last) = parsed.last_mut() {
                    token.clone_into(&mut last.1);
                }
            }
        }
    }
    parsed
}

/// Converts a scalar placeholder to a JSON value, preserving numbers.
fn scalar_value(value: &str) -> Value {
    let value = unquote(value);
    if let Ok(number) = value.parse::<u64>() {
        return Value::from(number);
    }
    Value::String(value.to_owned())
}
