//! Directional Serde renames in `CliValueExtractor`.
//!
//! A serialization-only rename makes a caller's serializer and the
//! configuration deserializer disagree. Extraction must read the serialized
//! spelling and write the configuration key, or the CLI layer carries a member
//! the final type silently ignores.

#[path = "support/default_punct.rs"]
mod default_punct;

use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::{CliValueExtractor, OrthoConfig};
use serde::{Deserialize, Serialize};

/// A parser type whose serialized key differs from its configuration key.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "directional-rename")]
#[ortho_config(prefix = "APP_")]
struct DirectionalRenameArgs {
    #[arg(long, default_value_t = default_punct::default_punct())]
    #[ortho_config(cli_default_as_absent)]
    #[serde(rename(serialize = "serialized-punctuation"))]
    punctuation: String,
}

impl Default for DirectionalRenameArgs {
    /// Build a `DirectionalRenameArgs` matching clap's own default.
    fn default() -> Self {
        Self {
            punctuation: default_punct::default_punct(),
        }
    }
}

/// A serialization-only rename is rekeyed for the CLI layer.
#[test]
fn test_extract_user_provided_rekeys_directional_rename() -> Result<()> {
    let matches = DirectionalRenameArgs::command().get_matches_from([
        "directional-rename",
        "--punctuation",
        "?",
    ]);
    let args =
        DirectionalRenameArgs::from_arg_matches(&matches).context("parse directional args")?;
    let extracted = args
        .extract_user_provided(&matches)
        .context("extract user provided")?;

    ensure!(
        extracted.get("punctuation").is_some(),
        "expected the configuration key to carry the value: {extracted:?}"
    );
    ensure!(
        extracted.get("serialized-punctuation").is_none(),
        "the serialization-only spelling must not reach the layer: {extracted:?}"
    );
    Ok(())
}
