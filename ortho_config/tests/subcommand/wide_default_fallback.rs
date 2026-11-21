//! The raw-extraction fallback for a document JSON cannot represent.
//!
//! `finish_with_required_values` projects the merged figment into JSON so the
//! absence collector can walk it, and falls back to the raw extraction when
//! that projection fails. A matches-aware loader seeds `T::default()`
//! unsanitised, so a default holding a `u128` above `u64::MAX` reaches the
//! loader as a document JSON cannot represent.
//!
//! These cases pin both halves of the documented outcome. A genuinely absent
//! required key still surfaces through the extraction error, which names the
//! first missing field rather than reporting a `MissingRequiredValues` payload
//! the loader could not compute, and a document that loads is never rejected
//! merely because enumeration could not run.

use anyhow::{Context as _, Result, anyhow, ensure};
use clap::{CommandFactory as _, FromArgMatches as _, Parser};
use ortho_config::{
    MapEnv, OrthoConfig, OrthoResult, SubcommandCliMatches, SubcommandFileContext,
    load_and_merge_subcommand_for_with_matches_with_sources_at,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::fixtures::{IsolatedRoot, isolated_root};
use super::merge::{wide_u128, write_scoped_section};
use super::to_anyhow::ToAnyhow;

/// A subcommand whose `Default` holds a value JSON cannot represent and whose
/// remaining required key no default supplies.
///
/// `wide` mirrors [`super::merge::MergeWideDefault`] on its own: the type's
/// `Default` is merged verbatim, and no layer can carry the value. `reference`
/// is the required key the projection would have reported if it could run; it
/// stays out of every layer because the parser skips it and the empty value it
/// leaves behind is skipped by serialization.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Parser, PartialEq)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergeWideDefaultWithAbsent {
    #[serde(deserialize_with = "wide_u128::deserialize")]
    #[arg(long, default_value_t = 0)]
    #[ortho_config(cli_default_as_absent)]
    wide: u128,
    #[arg(skip)]
    #[ortho_config(skip_cli)]
    #[serde(skip_serializing_if = "String::is_empty")]
    reference: String,
}

impl Default for MergeWideDefaultWithAbsent {
    fn default() -> Self {
        Self {
            wide: u128::MAX,
            reference: String::new(),
        }
    }
}

/// Run the schema-aware matches load over the fixture root.
///
/// The outer result reports a fixture that cannot build its CLI, which is a
/// bug in this case rather than a load verdict. The inner [`OrthoResult`] is
/// the loader's own result, kept separate so a case can inspect its variant.
fn load_matches(isolated: &IsolatedRoot) -> Result<OrthoResult<MergeWideDefaultWithAbsent>> {
    let discovery = isolated.discovery();
    let matches = MergeWideDefaultWithAbsent::command().get_matches_from(["test"]);
    let cli =
        MergeWideDefaultWithAbsent::from_arg_matches(&matches).context("parse wide-default CLI")?;
    let cli_matches = SubcommandCliMatches::new(&cli, &matches);
    Ok(load_and_merge_subcommand_for_with_matches_with_sources_at(
        &cli_matches,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    ))
}

/// An absent required key is reported by the extraction, not by the collector.
///
/// The document cannot be projected, so nothing can be enumerated. The load
/// must still fail on the absent key and must name it, rather than falling
/// through to a successful load or to an error about the projection.
#[rstest]
fn the_fallback_reports_the_first_missing_field(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let error = load_matches(&isolated)?
        .err()
        .ok_or_else(|| anyhow!("a required key absent from every layer must not load"))?;

    let message = error.to_string();
    ensure!(
        message.contains("missing field") && message.contains("reference"),
        "the extraction must name the first missing field, got: {message}"
    );
    ensure!(
        !matches!(
            error.as_ref(),
            ortho_config::OrthoError::MissingRequiredValues(_)
        ),
        "an unprojectable document must not report an enumerated payload, got: {message}"
    );
    Ok(())
}

/// A document that loads is accepted even though it cannot be enumerated.
///
/// Supplying the required key through its file route leaves the same
/// unprojectable `wide` default in the document. The fallback must accept the
/// load and keep the seeded value, so the missing enumeration never turns a
/// loadable configuration into an error.
#[rstest]
fn the_fallback_accepts_a_loadable_unprojectable_document(
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    write_scoped_section(&isolated, "reference = \"supplied\"")?;
    let merged = load_matches(&isolated)?
        .to_anyhow()
        .context("a supplied required key must load")?;

    ensure!(
        merged.reference == "supplied",
        "the file route must supply the required key, got {:?}",
        merged.reference
    );
    ensure!(
        merged.wide == u128::MAX,
        "the unprojectable default must survive the fallback, got {}",
        merged.wide
    );
    Ok(())
}
