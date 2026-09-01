//! Parity coverage for process-backed and injected `CsvEnv` providers.

use figment::{Jail, Provider};
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

fn configured_provider() -> CsvEnv {
    CsvEnv::prefixed("app_")
        .uppercase(true)
        .split("__")
        .lowercase(true)
}

fn assert_parity(pairs: &[(String, String)]) {
    Jail::expect_with(|jail| -> Result<(), figment::Error> {
        jail.clear_env();
        for (key, value) in pairs {
            jail.set_env(key, value);
        }

        let process = configured_provider().data()?;
        jail.clear_env();
        let injected =
            configured_provider().with_source(Arc::new(pairs.iter().cloned().collect::<MapEnv>()));

        assert_eq!(process, injected.data()?);
        Ok(())
    });
}

#[test]
fn injected_source_matches_the_process_backed_corpus() {
    assert_parity(
        &CORPUS
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect::<Vec<_>>(),
    );
}

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

proptest! {
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
