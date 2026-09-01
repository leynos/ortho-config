//! Parity coverage for process-backed and injected `CsvEnv` providers.

use figment::{
    Jail, Profile, Provider,
    value::{Dict, Map, Value},
};
use ortho_config::{CsvEnv, MapEnv};
use proptest::prelude::*;
use std::sync::Arc;

const CORPUS: &[(&str, &str)] = &[
    ("APP_DATABASE__HOST", "db.example.test"),
    ("app_SERVER__PORT", "5432"),
    ("APP_FEATURES", "alpha, beta, gamma"),
    ("APP_JSON", "[\"one,two\", \"three\"]"),
    ("APP_QUOTED", "\"one,two\""),
    ("APP_TRUE", "TRUE"),
    ("APP___EMPTY_COMPONENT", "dropped"),
    ("APP_", "dropped"),
    ("UNRELATED", "ignored"),
];

/// Build the corpus provider whose declarative setup mirrors generated loaders.
fn configured_provider() -> CsvEnv {
    CsvEnv::prefixed("app_")
        .uppercase(true)
        .split("__")
        .lowercase(true)
}

/// Check the shared corpus with the generated-loader transform sequence.
fn assert_parity(pairs: &[(String, String)]) {
    assert_provider_parity(configured_provider(), pairs);
}

/// Compare a process provider before clearing the jail with its injected replay.
///
/// Keeping those evaluations in separate jail states proves the injected path
/// reads only its supplied `MapEnv`, not variables retained by Figment's jail.
fn assert_provider_parity(provider: CsvEnv, pairs: &[(String, String)]) {
    Jail::expect_with(|jail| -> Result<(), figment::Error> {
        jail.clear_env();
        for (key, value) in pairs {
            jail.set_env(key, value);
        }

        let process = provider.data()?;
        jail.clear_env();
        let injected = provider.with_source(Arc::new(pairs.iter().cloned().collect::<MapEnv>()));

        assert_eq!(process, injected.data()?);
        Ok(())
    });
}

/// Figment maps builders in declaration order rather than grouping by mapping kind.
#[test]
fn interleaved_key_mappings_match_the_process_backed_provider() {
    let pairs = [(String::from("data-b"), String::from("7"))];

    assert_provider_parity(
        CsvEnv::raw().split("a").uppercase(true).lowercase(false),
        &pairs,
    );
}

/// Every Figment key mapping restores its default lowercase mode.
#[test]
fn key_mapping_resets_lowercase_like_the_process_backed_provider() {
    let pairs = [(String::from("MIXED_CASE"), String::from("7"))];

    assert_provider_parity(CsvEnv::raw().lowercase(false).split("_"), &pairs);
}

/// A later lowercase builder remains able to opt out after a key mapping reset.
#[test]
fn lowercase_can_be_disabled_after_a_key_mapping() {
    let pairs = [(String::from("MIXED_CASE"), String::from("7"))];

    assert_provider_parity(CsvEnv::raw().split("_").lowercase(false), &pairs);
}

/// Return the default-profile dictionary from a provider result.
fn default_dict(data: &Map<Profile, Dict>) -> &Dict {
    data.get(&Profile::Default).map_or_else(
        || panic!("CsvEnv providers always collect into the default profile"),
        |dictionary| dictionary,
    )
}

/// Assert the recursively merged database value retains both sibling keys.
fn assert_database_siblings(data: &Map<Profile, Dict>) {
    let database = default_dict(data)
        .get("database")
        .and_then(Value::as_dict)
        .map_or_else(
            || panic!("database must be a nested dictionary"),
            |dictionary| dictionary,
        );
    assert_eq!(
        database.get("host").and_then(Value::as_str),
        Some("db.example.test")
    );
    assert_eq!(database.get("port").and_then(Value::to_u128), Some(5432));
}

/// Cover nesting, scalar parsing, CSV handling, and process isolation together.
#[test]
fn injected_source_matches_the_process_backed_corpus() {
    assert_parity(
        &CORPUS
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect::<Vec<_>>(),
    );
}

/// Process and injected paths recursively merge sibling nested keys alike.
#[test]
fn nested_siblings_are_preserved_in_both_paths() {
    let pairs = [
        (
            String::from("APP_DATABASE__HOST"),
            String::from("db.example.test"),
        ),
        (String::from("APP_DATABASE__PORT"), String::from("5432")),
    ];

    Jail::expect_with(|jail| -> Result<(), figment::Error> {
        jail.clear_env();
        for (key, value) in &pairs {
            jail.set_env(key, value);
        }

        let process = CsvEnv::prefixed("APP_").split("__").data()?;
        jail.clear_env();
        let injected = CsvEnv::prefixed("APP_")
            .split("__")
            .with_source(Arc::new(pairs.into_iter().collect::<MapEnv>()))
            .data()?;

        assert_eq!(process, injected);
        assert_database_siblings(&process);
        assert_database_siblings(&injected);
        Ok(())
    });
}

/// Replaying split builders in call order matches Figment's chained mappings.
#[test]
fn chained_split_patterns_match_the_process_backed_mapping() {
    let pairs = [(String::from("APP_A_B-C"), String::from("7"))];

    Jail::expect_with(|jail| -> Result<(), figment::Error> {
        jail.clear_env();
        jail.set_env("APP_A_B-C", "7");

        let process = CsvEnv::prefixed("APP_").split("_").split("-").data()?;
        jail.clear_env();
        let injected = CsvEnv::prefixed("APP_")
            .split("_")
            .split("-")
            .with_source(Arc::new(pairs.into_iter().collect::<MapEnv>()))
            .data()?;

        assert_eq!(process, injected);
        let a = default_dict(&injected)
            .get("a")
            .and_then(Value::as_dict)
            .expect("first split component must be a dictionary");
        let b = a
            .get("b")
            .and_then(Value::as_dict)
            .expect("second split component must be a dictionary");
        assert_eq!(b.get("c").and_then(Value::to_u128), Some(7));
        Ok(())
    });
}

/// Preserve a comma-containing scalar when both providers disable CSV parsing.
#[test]
fn csv_can_be_disabled_for_an_injected_source() {
    let pairs = [(String::from("APP_VALUES"), String::from("one,two"))];

    Jail::expect_with(|jail| -> Result<(), figment::Error> {
        jail.clear_env();
        jail.set_env("APP_VALUES", "one,two");

        let process = CsvEnv::prefixed("APP_").csv(false);
        let process_data = process.data()?;
        jail.clear_env();
        let injected = CsvEnv::prefixed("APP_")
            .csv(false)
            .with_source(Arc::new(pairs.iter().cloned().collect::<MapEnv>()));

        assert_eq!(process_data, injected.data()?);
        Ok(())
    });
}

/// Reject unreplayable key closures before injected scanning can change semantics.
#[test]
fn injected_source_rejects_opaque_key_transforms() {
    let error = CsvEnv::raw()
        .map(|key| key.into())
        .with_source(Arc::new(MapEnv::new().with_var("APP_HOST", "localhost")))
        .data()
        .expect_err("injected arbitrary key transforms must be rejected");

    assert!(
        error
            .to_string()
            .contains("injected ScanEnvSource after map or filter_map"),
        "unexpected error: {error}"
    );
}

/// An injected source also rejects a `filter_map` closure it cannot replay.
#[test]
fn injected_source_rejects_opaque_filter_map_transforms() {
    let error = CsvEnv::raw()
        .filter_map(|key| Some(key.into()))
        .with_source(Arc::new(MapEnv::new().with_var("APP_HOST", "localhost")))
        .data()
        .expect_err("injected arbitrary key transforms must be rejected");

    assert!(
        error
            .to_string()
            .contains("injected ScanEnvSource after map or filter_map"),
        "unexpected error: {error}"
    );
}

proptest! {
    /// Exercise generated key and value pairs without mutating process state.
    #[test]
    fn injected_source_matches_process_backed_generated_pairs(
        pairs in prop::collection::vec(
            (
                proptest::string::string_regex("[A-Za-z0-9_]{1,24}")
                    .expect("key regex must be valid"),
                proptest::string::string_regex("[A-Za-z0-9_ ,\\[\\]\\{\\}\"']{0,40}")
                    .expect("value regex must be valid"),
            ),
            0..24,
        ),
    ) {
        assert_parity(&pairs);
    }
}
