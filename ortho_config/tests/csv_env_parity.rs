//! Parity coverage for process-backed and injected `CsvEnv` providers.

use anyhow::{Context, Result, anyhow, ensure};
use figment::{
    Profile, Provider,
    value::{Dict, Map, Value},
};
use ortho_config::{CsvEnv, MapEnv};
use proptest::prelude::*;
use std::{io::Write, process::Command, sync::Arc};

const PROCESS_DATA_MARKER: &str = "ORTHO_CSV_PROCESS_DATA:";
/// Keep process probes scoped to supplied keys, including under coverage.
const PROCESS_PROBE_PREFIX: &str = "PARITY_";

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
    assert_provider_parity(configured_provider(), "configured_process_probe", pairs)
}

/// Compare a process provider in a fresh child with its injected replay here.
fn assert_provider_parity(provider: CsvEnv, probe: &str, pairs: &[(String, String)]) -> Result<()> {
    assert_provider_parity_with(provider, probe, pairs, |_| Ok(()))
}

/// Run an optional test-specific assertion after establishing provider parity.
///
/// The child's environment contains only the supplied pairs. The injected
/// provider runs in this process, so it cannot inherit the child's variables.
fn assert_provider_parity_with<F>(
    provider: CsvEnv,
    probe: &str,
    pairs: &[(String, String)],
    check: F,
) -> Result<()>
where
    F: FnOnce(&Map<Profile, Dict>) -> Result<()>,
{
    let process_data = process_provider_data(probe, pairs)?;
    let injected = provider.with_source(Arc::new(pairs.iter().cloned().collect::<MapEnv>()));
    let injected_data = injected.data()?;

    ensure!(
        process_data == format!("{injected_data:?}"),
        "process and injected providers differ: process={process_data}, injected={injected_data:?}"
    );
    check(&injected_data)
}

/// Read a provider result from a child with an isolated process environment.
fn process_provider_data(probe: &str, pairs: &[(String, String)]) -> Result<String> {
    let executable = std::env::current_exe().context("locate the integration-test executable")?;
    let output = Command::new(executable)
        .args(["--ignored", "--exact", "--show-output", probe])
        .env_clear()
        .envs(pairs.iter().cloned())
        .output()
        .context("run the process-backed CsvEnv probe")?;
    ensure!(
        output.status.success(),
        "process-backed CsvEnv probe `{probe}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("decode CsvEnv probe output")?;
    let results = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(PROCESS_DATA_MARKER))
        .collect::<Vec<_>>();
    match results.as_slice() {
        [data] => Ok((*data).to_owned()),
        _ => Err(anyhow!(
            "process-backed CsvEnv probe `{probe}` emitted {} results: {stdout}",
            results.len()
        )),
    }
}

/// Emit the process provider's exact Figment value tree for its parent test.
fn emit_process_data(provider: &CsvEnv) -> Result<()> {
    let data = provider.data()?;
    writeln!(std::io::stdout().lock(), "{PROCESS_DATA_MARKER}{data:?}")
        .context("write process-backed CsvEnv result")
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn configured_process_probe() {
    emit_process_data(&configured_provider()).expect("emit configured process provider data");
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn interleaved_process_probe() {
    emit_process_data(
        &CsvEnv::prefixed(PROCESS_PROBE_PREFIX)
            .split("a")
            .uppercase(true)
            .lowercase(false),
    )
    .expect("emit interleaved process provider data");
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn reset_lowercase_process_probe() {
    emit_process_data(
        &CsvEnv::prefixed(PROCESS_PROBE_PREFIX)
            .lowercase(false)
            .split("_"),
    )
    .expect("emit reset-lowercase process provider data");
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn disabled_lowercase_process_probe() {
    emit_process_data(
        &CsvEnv::prefixed(PROCESS_PROBE_PREFIX)
            .split("_")
            .lowercase(false),
    )
    .expect("emit disabled-lowercase process provider data");
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn nested_process_probe() {
    emit_process_data(&CsvEnv::prefixed("APP_").split("__"))
        .expect("emit nested process provider data");
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn chained_split_process_probe() {
    emit_process_data(&CsvEnv::prefixed("APP_").split("_").split("-"))
        .expect("emit chained-split process provider data");
}

#[test]
#[ignore = "run by parity tests with an isolated child environment"]
fn no_csv_process_probe() {
    emit_process_data(&CsvEnv::prefixed("APP_").csv(false))
        .expect("emit no-CSV process provider data");
}

/// Figment maps builders in declaration order rather than grouping by mapping kind.
#[test]
fn interleaved_key_mappings_match_the_process_backed_provider() {
    let pairs = [(format!("{PROCESS_PROBE_PREFIX}data-b"), String::from("7"))];

    assert_provider_parity(
        CsvEnv::prefixed(PROCESS_PROBE_PREFIX)
            .split("a")
            .uppercase(true)
            .lowercase(false),
        "interleaved_process_probe",
        &pairs,
    )
    .expect("interleaved key mappings should match the process-backed provider");
}

/// Every Figment key mapping restores its default lowercase mode.
#[test]
fn key_mapping_resets_lowercase_like_the_process_backed_provider() {
    let pairs = [(
        format!("{PROCESS_PROBE_PREFIX}MIXED_CASE"),
        String::from("7"),
    )];

    assert_provider_parity(
        CsvEnv::prefixed(PROCESS_PROBE_PREFIX)
            .lowercase(false)
            .split("_"),
        "reset_lowercase_process_probe",
        &pairs,
    )
    .expect("key mappings should reset lowercase mode");
}

/// A later lowercase builder remains able to opt out after a key mapping reset.
#[test]
fn lowercase_can_be_disabled_after_a_key_mapping() {
    let pairs = [(
        format!("{PROCESS_PROBE_PREFIX}MIXED_CASE"),
        String::from("7"),
    )];

    assert_provider_parity(
        CsvEnv::prefixed(PROCESS_PROBE_PREFIX)
            .split("_")
            .lowercase(false),
        "disabled_lowercase_process_probe",
        &pairs,
    )
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
        "nested_process_probe",
        &pairs,
        assert_database_siblings,
    )
    .expect("nested siblings should be preserved in both provider paths");
}

/// Replaying split builders in call order matches Figment's chained mappings.
#[test]
fn chained_split_patterns_match_the_process_backed_mapping() {
    let pairs = [(String::from("APP_A_B-C"), String::from("7"))];

    assert_provider_parity_with(
        CsvEnv::prefixed("APP_").split("_").split("-"),
        "chained_split_process_probe",
        &pairs,
        |injected| {
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

    assert_provider_parity(
        CsvEnv::prefixed("APP_").csv(false),
        "no_csv_process_probe",
        &pairs,
    )
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
