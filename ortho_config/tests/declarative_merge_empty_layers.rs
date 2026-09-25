//! Regression coverage for merges in which no layer contributes a value.
//!
//! The generated declarative state seeds its accumulator with
//! `serde_json::Value::default()`, which is `Null`, and `merge_layer` skips
//! empty maps. A layer list that supplied nothing therefore left `finish`
//! deserializing `Null` straight into the configuration struct, which fails
//! with `invalid type: null, expected struct …` rather than producing a value.
//!
//! This is reachable from the ordinary `load` path, not just from a synthetic
//! empty layer list: a prefixed struct whose every field is optional reads
//! nothing from the environment, and a missing configuration file contributes
//! no layer, so nothing ever seats the accumulator. Unprefixed structs masked
//! the defect because `CsvEnv::raw()` always yields a non-empty object.

use anyhow::{Result, ensure};
use ortho_config::{MergeLayer, OrthoConfig};
use rstest::rstest;
use serde::Deserialize;
use test_helpers::figment as figment_helpers;

/// Every field may legitimately be absent, so a merge that supplies nothing
/// still has a well-defined result: all fields `None`.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "EMPTYLAYER_")]
struct AllOptional {
    flag: Option<bool>,
    name: Option<String>,
}

/// Negative control: a required field must still be reported as missing rather
/// than silently defaulted, so the fix must not turn absence into a value for
/// every field.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "EMPTYLAYER_")]
struct HasRequired {
    #[expect(
        dead_code,
        reason = "the field exists to be reported missing, never to be read"
    )]
    name: String,
}

/// An empty layer list must yield an all-`None` configuration, not an error.
///
/// `merge_from_layers([])` is the minimal reproduction: it reaches `finish`
/// without ever calling `merge_layer`, so the accumulator is still at its
/// initial `Null`.
#[rstest]
fn no_layers_yields_all_absent_fields() -> Result<()> {
    let layers: Vec<MergeLayer<'static>> = Vec::new();
    let config = AllOptional::merge_from_layers(layers)?;
    ensure!(
        config.flag.is_none(),
        "expected flag absent, got {:?}",
        config.flag
    );
    ensure!(
        config.name.is_none(),
        "expected name absent, got {:?}",
        config.name
    );
    Ok(())
}

/// The same merge reached through the generated `load` path, which is how a
/// user meets it: a prefixed, all-optional struct with no environment value
/// and no configuration file to read.
#[rstest]
fn load_without_any_source_leaves_every_field_absent() -> Result<()> {
    figment_helpers::with_jail(|jail| {
        jail.clear_env();
        let config = AllOptional::load_from_iter(["prog"])
            .map_err(|err| figment_helpers::figment_error(err.to_string()))?;
        if config.flag.is_some() || config.name.is_some() {
            return Err(figment_helpers::figment_error(format!(
                "expected every field absent, got {config:?}"
            )));
        }
        Ok(())
    })?;
    Ok(())
}

/// A required field must still fail as a missing field once the accumulator
/// starts from an empty object, and the message must say so.
#[rstest]
fn required_field_is_still_reported_missing() -> Result<()> {
    let layers: Vec<MergeLayer<'static>> = Vec::new();
    let error = HasRequired::merge_from_layers(layers)
        .expect_err("a missing required field must not merge successfully");
    let message = error.to_string();
    ensure!(
        message.contains("missing field"),
        "expected a missing-field error, got: {message}"
    );
    Ok(())
}
