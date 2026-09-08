//! Parity coverage for process-backed and injected `CsvEnv` providers.

use anyhow::{Result, anyhow, ensure};
use figment::{
    Profile, Provider,
    value::{Dict, Map, Value},
};
use ortho_config::{CsvEnv, MapEnv};
use proptest::prelude::*;
use std::sync::Arc;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

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
fn assert_parity(pairs: &[(String, String)]) -> Result<()> {
    assert_provider_parity(configured_provider(), pairs)
}

/// Compare a process provider before clearing the jail with its injected replay.
///
/// Keeping those evaluations in separate jail states proves the injected path
/// reads only its supplied `MapEnv`, not variables retained by Figment's jail.
fn assert_provider_parity(provider: CsvEnv, pairs: &[(String, String)]) -> Result<()> {
    assert_provider_parity_with(provider, pairs, |_, _| Ok(()))
}

/// Run an optional test-specific assertion after establishing provider parity.
///
/// The callback receives data captured from the process-backed provider before
/// Figment's jail is cleared, then data from the injected provider after it is
/// cleared. This ordering keeps the injected-source isolation invariant shared
/// by every parity test in one place.
fn assert_provider_parity_with<F>(
    provider: CsvEnv,
    pairs: &[(String, String)],
    check: F,
) -> Result<()>
where
    F: FnOnce(&Map<Profile, Dict>, &Map<Profile, Dict>) -> Result<()>,
{
    with_jail(|jail| {
        jail.clear_env();
        for (key, value) in pairs {
            jail.set_env(key, value);
        }

        let process = provider.data()?;
        jail.clear_env();
        let injected = provider.with_source(Arc::new(pairs.iter().cloned().collect::<MapEnv>()));

        let injected_data = injected.data()?;

        ensure!(
            process == injected_data,
            "process and injected providers differ"
        );
        check(&process, &injected_data)?;
        Ok(())
    })
}

/// Figment maps builders in declaration order rather than grouping by mapping kind.
#[test]
fn interleaved_key_mappings_match_the_process_backed_provider() {
    let pairs = [(String::from("data-b"), String::from("7"))];

    assert_provider_parity(
        CsvEnv::raw().split("a").uppercase(true).lowercase(false),
        &pairs,
    )
    .expect("interleaved key mappings should match the process-backed provider");
}

/// Every Figment key mapping restores its default lowercase mode.
#[test]
fn key_mapping_resets_lowercase_like_the_process_backed_provider() {
    let pairs = [(String::from("MIXED_CASE"), String::from("7"))];

    assert_provider_parity(CsvEnv::raw().lowercase(false).split("_"), &pairs)
        .expect("key mappings should reset lowercase mode");
}

/// A later lowercase builder remains able to opt out after a key mapping reset.
#[test]
fn lowercase_can_be_disabled_after_a_key_mapping() {
    let pairs = [(String::from("MIXED_CASE"), String::from("7"))];

    assert_provider_parity(CsvEnv::raw().split("_").lowercase(false), &pairs)
        .expect("lowercase should remain disabled after a key mapping");
}

/// Return the default-profile dictionary from a provider result.
fn default_dict(data: &Map<Profile, Dict>) -> Result<&Dict> {
    data.get(&Profile::Default)
        .ok_or_else(|| anyhow!("CsvEnv providers always collect into the default profile"))
}

/// Assert the recursively merged database value retains both sibling keys.
fn assert_database_siblings(data: &Map<Profile, Dict>) -> Result<()> {
    let database = default_dict(data)?
        .get("database")
        .and_then(Value::as_dict)
        .ok_or_else(|| anyhow!("database must be a nested dictionary"))?;
    ensure!(
        database.get("host").and_then(Value::as_str) == Some("db.example.test"),
        "database host should be preserved"
    );
    ensure!(
        database.get("port").and_then(Value::to_u128) == Some(5432),
        "database port should be preserved"
    );
    Ok(())
}

/// Cover nesting, scalar parsing, CSV handling, and process isolation together.
#[test]
fn injected_source_matches_the_process_backed_corpus() {
    assert_parity(
        &CORPUS
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect::<Vec<_>>(),
    )
    .expect("injected source should match the process-backed corpus");
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

    assert_provider_parity_with(
        CsvEnv::prefixed("APP_").split("__"),
        &pairs,
        |process, injected| {
            assert_database_siblings(process)?;
            assert_database_siblings(injected)
        },
    )
    .expect("nested siblings should be preserved in both provider paths");
}

/// Replaying split builders in call order matches Figment's chained mappings.
#[test]
fn chained_split_patterns_match_the_process_backed_mapping() {
    let pairs = [(String::from("APP_A_B-C"), String::from("7"))];

    assert_provider_parity_with(
        CsvEnv::prefixed("APP_").split("_").split("-"),
        &pairs,
        |_, injected| {
            let a = default_dict(injected)?
                .get("a")
                .and_then(Value::as_dict)
                .ok_or_else(|| anyhow!("first split component must be a dictionary"))?;
            let b = a
                .get("b")
                .and_then(Value::as_dict)
                .ok_or_else(|| anyhow!("second split component must be a dictionary"))?;
            ensure!(
                b.get("c").and_then(Value::to_u128) == Some(7),
                "chained split should preserve the final value"
            );
            Ok(())
        },
    )
    .expect("chained split patterns should match the process-backed mapping");
}

/// Preserve a comma-containing scalar when both providers disable CSV parsing.
#[test]
fn csv_can_be_disabled_for_an_injected_source() {
    let pairs = [(String::from("APP_VALUES"), String::from("one,two"))];

    assert_provider_parity(CsvEnv::prefixed("APP_").csv(false), &pairs)
        .expect("CSV disabling should preserve the injected scalar");
}

/// Reject unreplayable key closures before injected scanning can change semantics.
///
/// Both `map` variants must fail before scanning because their closures cannot
/// be replayed safely against an injected source.
fn assert_injected_source_rejects_opaque_transform(transform: impl FnOnce(CsvEnv) -> CsvEnv) {
    let result = transform(CsvEnv::raw())
        .with_source(Arc::new(MapEnv::new().with_var("APP_HOST", "localhost")))
        .data();
    let Err(error) = result else {
        panic!("injected arbitrary key transforms must be rejected");
    };

    assert!(
        error
            .to_string()
            .contains("injected ScanEnvSource after map or filter_map"),
        "unexpected error: {error}"
    );
}

/// Reject a `map` closure that injected loading cannot replay.
#[test]
fn injected_source_rejects_opaque_key_transforms() {
    assert_injected_source_rejects_opaque_transform(|provider| provider.map(|key| key.into()));
}

/// An injected source also rejects a `filter_map` closure it cannot replay.
#[test]
fn injected_source_rejects_opaque_filter_map_transforms() {
    assert_injected_source_rejects_opaque_transform(|provider| {
        provider.filter_map(|key| Some(key.into()))
    });
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
        assert_parity(&pairs).expect("generated pairs should preserve provider parity");
    }
}
