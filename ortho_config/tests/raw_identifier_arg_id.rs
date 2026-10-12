//! Agreement between the derived clap argument id and clap's own id for a
//! raw-identifier field.
//!
//! `clap_derive` derives the argument id for a field with no explicit
//! `id = "..."` from `ident.unraw().to_string()`, so a field written as
//! `r#type` is recorded by clap under the id `type`. The `OrthoConfig` derive
//! must build the same string, because it looks the argument up again in
//! `ArgMatches`: `CliValueExtractor` queries `value_source(#arg_id)` for every
//! `cli_default_as_absent` field.
//!
//! If the derive derived the fallback from `Ident::to_string()` instead, it
//! would emit `r#type`, `ArgMatches::value_source` would return `None` for a
//! flag the user did supply, and a value the user typed on the command line
//! would be silently dropped in favour of the file layer. That failure is
//! invisible without a test that actually parses a raw-identifier flag, which
//! is what this module provides.

use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::{CliValueExtractor, OrthoConfig};
use serde::{Deserialize, Serialize};

#[path = "support/default_punct.rs"]
mod default_punct;

/// A clap-only raw-identifier field, with no `OrthoConfig` derive.
///
/// This fixture isolates upstream behaviour from anything this crate derives.
/// `r#type` is the spelling a struct needs when a field name collides with a
/// Rust keyword, so it is expected in real configuration structs rather than
/// being a contrived shape: `clap_derive` builds the id from
/// `ident.unraw().to_string()` and kebab-cases only the *long flag*, giving
/// the id `type` and the flag `--type`.
#[derive(Debug, Parser)]
#[command(name = "raw-ident-upstream")]
struct UpstreamRawIdent {
    #[arg(long)]
    r#type: String,
}

/// A raw-identifier field carrying a clap default, through the full derive.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "raw-ident")]
#[ortho_config(prefix = "APP_")]
struct RawIdentArgs {
    /// Punctuation "type" is a Rust keyword, so the field is written raw.
    #[serde(rename = "type")]
    #[arg(long, default_value_t = default_punct::default_punct())]
    #[ortho_config(cli_default_as_absent)]
    r#type: String,
}

impl Default for RawIdentArgs {
    /// Build a `RawIdentArgs` holding the value clap applies by default.
    fn default() -> Self {
        Self {
            r#type: default_punct::default_punct(),
        }
    }
}

/// clap records the raw-identifier field under the unrawed id and flag.
///
/// This pins the upstream behaviour the derive has to match. If a future clap
/// release changed it, the derive's fallback would be wrong and this test
/// would say so at the point of the change rather than through a silently
/// dropped CLI value.
#[test]
fn clap_records_the_unrawed_id_for_a_raw_identifier_field() {
    let ids = UpstreamRawIdent::command()
        .get_arguments()
        .map(|arg| arg.get_id().to_string())
        .collect::<Vec<_>>();

    ensure_contains(&ids, "type");
    assert!(
        !ids.iter().any(|id| id.starts_with("r#")),
        "clap must not record the raw spelling; got {ids:?}",
    );

    let mut command = UpstreamRawIdent::command();
    let help = command.render_long_help().to_string();
    assert!(
        help.contains("--type"),
        "clap must derive the unrawed long flag; help was:\n{help}",
    );
}

/// An explicit raw-identifier flag is attributed to the command line.
///
/// This is the derive/runtime agreement assertion: `--type` is parsed by clap
/// under the id `type`, and the generated extractor finds it there. A fallback
/// built from `Ident::to_string()` fails here, because `value_source("r#type")`
/// returns `None` for an argument clap recorded as `type`.
#[test]
fn explicit_raw_identifier_flag_is_extracted() -> Result<()> {
    let extracted = parse_and_extract(["raw-ident", "--type", "!!!"])?;

    ensure!(
        extracted.get("type").is_some(),
        "expected the command-line value to be attributed to the caller, got: {extracted:?}",
    );

    Ok(())
}

/// An absent raw-identifier flag keeps clap's default out of the CLI layer.
///
/// `cli_default_as_absent` exists so file and environment layers can win over
/// a clap default. A resolver that matched the *wrong* id would be caught here
/// as well: matching nothing is indistinguishable from matching a default, so
/// the pair of tests is what separates "found the caller's value" from "found
/// nothing at all".
#[test]
fn absent_raw_identifier_flag_leaves_the_clap_default_out() -> Result<()> {
    let extracted = parse_and_extract(["raw-ident"])?;

    ensure!(
        extracted.get("type").is_none(),
        "clap's default must not be attributed to the caller, got: {extracted:?}",
    );

    Ok(())
}

/// Parses `cli_args` into [`RawIdentArgs`] and extracts the caller-supplied
/// fields.
fn parse_and_extract<I, S>(cli_args: I) -> Result<serde_json::Value>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let matches = RawIdentArgs::command().get_matches_from(cli_args);
    let args = RawIdentArgs::from_arg_matches(&matches).context("parse CLI args")?;
    args.extract_user_provided(&matches)
        .context("extract user provided")
}

/// Asserts `ids` contains `expected`, naming the observed ids on failure.
fn ensure_contains(ids: &[String], expected: &str) {
    assert!(
        ids.iter().any(|id| id == expected),
        "expected arg id {expected:?}; got {ids:?}",
    );
}
