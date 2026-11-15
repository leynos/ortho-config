//! The configuration file value the profile scenarios write.
//!
//! Split from `profiles_steps` to keep both modules within the repository's
//! 400-line ceiling. Everything here turns the keys a scenario accumulated into
//! the JSON value that is serialised to the scenario's config file.

use crate::rstest_bdd_support::scenario_state::ProfilesContext;
use serde_json::{Map, Value};

use super::profile_placeholders::TABLE_SENTINEL;
use super::value_parsing::unquote;

/// Builds the file value from the accumulated base keys and profile tables.
pub(super) fn build_file_value(profiles_context: &ProfilesContext) -> Value {
    let mut file = Map::new();
    let base_keys = profiles_context.base_keys.get().unwrap_or_default();
    for (key, value) in base_keys {
        file.insert(key, scalar_value(&value));
    }
    let profile_keys = profiles_context.profile_keys.get().unwrap_or_default();
    if !profile_keys.is_empty() {
        file.insert("profile".to_owned(), build_profile_tables(profile_keys));
    }
    Value::Object(file)
}

/// Groups `(profile, key, value)` triples into one table per profile.
///
/// An empty key adds nothing but still creates its profile's table, and the
/// table sentinel becomes an empty nested table rather than a scalar.
fn build_profile_tables(profile_keys: Vec<(String, String, String)>) -> Value {
    let mut profiles = Map::new();
    for (profile, key, value) in profile_keys {
        let table = profiles
            .entry(profile)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(table_map) = table {
            insert_profile_key(table_map, key, &value);
        }
    }
    Value::Object(profiles)
}

/// Inserts one profile key, skipping an empty key.
fn insert_profile_key(table: &mut Map<String, Value>, key: String, value: &str) {
    if key.is_empty() {
        return;
    }
    let entry = if value == TABLE_SENTINEL {
        Value::Object(Map::new())
    } else {
        scalar_value(value)
    };
    table.insert(key, entry);
}

/// Converts a scalar placeholder to a JSON value, preserving its type.
///
/// Numbers and booleans become their native JSON forms so the file written for
/// a scenario deserializes into the field's declared type; a bool written as a
/// TOML string would fail to deserialize rather than exercising precedence.
fn scalar_value(value: &str) -> Value {
    let value = unquote(value);
    if let Ok(number) = value.parse::<u64>() {
        return Value::from(number);
    }
    match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(value.to_owned()),
    }
}
