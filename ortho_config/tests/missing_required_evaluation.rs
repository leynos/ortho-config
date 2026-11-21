//! Double-evaluation guard for schema composition and collection.
//!
//! The plan requires evidence that composing the requirement schema and
//! collecting missing values read the already-merged value without replaying
//! defaults or custom deserialization. Two instrumented hooks count their own
//! evaluations: a handwritten `Deserialize` implementation and an
//! `#[ortho_config(default = ...)]` expression.
//!
//! A load that stops at a missing requirement must not deserialize the
//! configuration at all. A successful load must evaluate each hook exactly
//! once. Composing and collecting through the public entry point must not
//! touch either hook, because it only inspects a value that already exists.

use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig, OrthoError, ensure_no_missing_required_values};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The invoked program name every load uses.
const PROGRAM: &str = "instrumented";

/// Counts `Counted::deserialize` invocations in this binary.
static DESERIALIZATIONS: AtomicUsize = AtomicUsize::new(0);

/// Counts `counted_default` invocations in this binary.
static DEFAULT_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

/// Clear both counters and return their values before they were cleared.
fn take_counts() -> (usize, usize) {
    (
        DESERIALIZATIONS.swap(0, Ordering::SeqCst),
        DEFAULT_EVALUATIONS.swap(0, Ordering::SeqCst),
    )
}

/// A scalar that records every deserialization of itself.
#[derive(Debug, Serialize)]
struct Counted(String);

impl<'de> Deserialize<'de> for Counted {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DESERIALIZATIONS.fetch_add(1, Ordering::SeqCst);
        String::deserialize(deserializer).map(Self)
    }
}

/// A documented default that records every evaluation of itself.
fn counted_default() -> u16 {
    DEFAULT_EVALUATIONS.fetch_add(1, Ordering::SeqCst);
    8080
}

/// A configuration instrumenting one custom leaf and one defaulted leaf.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRV_")]
struct InstrumentedRoot {
    /// Required, and deserialized by the instrumented implementation.
    ///
    /// The parser is skipped so the fixture needs no clap value parser; the
    /// value arrives through the injected environment instead.
    #[ortho_config(skip_cli)]
    counted: Counted,
    /// Optional because an `ortho_config` default exists.
    #[ortho_config(default = counted_default())]
    port: u16,
    /// Required plain scalar, left absent to force the error path.
    reference: String,
}

/// Load [`InstrumentedRoot`] through the generated source-aware entry point.
fn load(vars: &[(&str, &str)]) -> ortho_config::OrthoResult<InstrumentedRoot> {
    let mut environment = MapEnv::new();
    for (key, value) in vars {
        environment.insert(*key, *value);
    }
    let source = Arc::new(environment);
    InstrumentedRoot::load_from_iter_with_sources([PROGRAM], source.clone(), source)
}

/// Assert the counters report `deserializations` and `default_evaluations`.
fn assert_counts(deserializations: usize, default_evaluations: usize, phase: &str) {
    let actual = take_counts();
    assert_eq!(
        actual,
        (deserializations, default_evaluations),
        "{phase} must evaluate the deserializer {deserializations} times and the default \
         {default_evaluations} times"
    );
}

#[test]
fn custom_hooks_are_never_evaluated_twice() -> Result<()> {
    take_counts();

    let error = load(&[("MRV_COUNTED", "c")]).expect_err("the reference is absent");
    let rendered = match error.as_ref() {
        OrthoError::MissingRequiredValues(report) => report.to_string(),
        other => return Err(anyhow!("expected missing values, got: {other}")),
    };
    ensure!(
        rendered.contains("reference"),
        "the diagnostic must name the absent key: {rendered}"
    );
    assert_counts(0, 1, "a load that stops at a missing value");

    let config = load(&[("MRV_COUNTED", "c"), ("MRV_REFERENCE", "r")])
        .map_err(|failure| anyhow!("{failure}"))?;
    ensure!(config.counted.0 == "c", "the counted leaf must be read");
    ensure!(config.port == 8080, "the default must be applied");
    assert_counts(1, 1, "a successful load");

    ensure_no_missing_required_values::<InstrumentedRoot>(
        &json!({"counted": "c", "port": 1, "reference": "r"}),
        PROGRAM,
    )
    .map_err(|failure| anyhow!("{failure}"))?;
    assert_counts(0, 0, "schema composition and collection alone");

    Ok(())
}
