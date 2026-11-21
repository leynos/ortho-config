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

use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, MergeLayer, OrthoConfig};
use rstest::rstest;
use serde::Deserialize;
use std::sync::Arc;

#[path = "support/hermetic_env.rs"]
mod hermetic_env;

use hermetic_env::hermetic;

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
    // A hermetic source pins the working directory and the XDG bases to an
    // empty root, so neither the host's files nor the process's directory can
    // join the load.
    let root = tempfile::tempdir().context("create the empty root")?;
    let env = Arc::new(hermetic(MapEnv::new(), root.path()));
    let config = AllOptional::load_from_iter_with_env(["prog"], env).map_err(|err| anyhow!(err))?;
    ensure!(
        config.flag.is_none() && config.name.is_none(),
        "expected every field absent, got {config:?}"
    );
    Ok(())
}

/// A matching project file in *another* directory must not be found.
///
/// A positive control first loads the decoy by pinning the working directory
/// to the directory that holds it, so the decoy is known to be readable. The
/// same source then goes through `hermetic` with an empty root, which must
/// pin the directory away from it again: only that pin keeps it out of the
/// load.
#[rstest]
fn a_project_file_outside_the_pinned_directory_is_ignored() -> Result<()> {
    let elsewhere = tempfile::tempdir().context("create the directory holding the file")?;
    Dir::open_ambient_dir(elsewhere.path(), ambient_authority())
        .context("open the directory holding the file")?
        .write(".emptylayer.toml", "flag = true\n")
        .context("write the decoy")?;
    let root = tempfile::tempdir().context("create the empty root")?;
    let with_decoy = MapEnv::new().with_current_dir(elsewhere.path());

    let control = AllOptional::load_from_iter_with_env(["prog"], Arc::new(with_decoy.clone()))
        .map_err(|err| anyhow!(err))?;
    ensure!(
        control.flag == Some(true),
        "the decoy must be readable through its own directory: {control:?}"
    );

    let pinned = Arc::new(hermetic(with_decoy, root.path()));
    let config =
        AllOptional::load_from_iter_with_env(["prog"], pinned).map_err(|err| anyhow!(err))?;
    ensure!(config.flag.is_none(), "the decoy file was read: {config:?}");
    Ok(())
}

/// A required field must still fail as a missing field once the accumulator
/// starts from an empty object, and the enumerated diagnostic must name it.
#[rstest]
fn required_field_is_still_reported_missing() -> Result<()> {
    let layers: Vec<MergeLayer<'static>> = Vec::new();
    let error = HasRequired::merge_from_layers(layers)
        .expect_err("a missing required field must not merge successfully");
    let message = error.to_string();
    ensure!(
        message.contains("Missing required configuration") && message.contains("`name`"),
        "expected an enumerated missing-value error, got: {message}"
    );
    Ok(())
}
