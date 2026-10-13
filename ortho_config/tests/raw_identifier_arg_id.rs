//! Agreement between generated clap identifiers and clap's own for a
//! raw-identifier field.
//!
//! A field whose name collides with a Rust keyword must be written raw, so
//! `r#type` and `r#match` are ordinary spellings in a real configuration
//! struct rather than contrived shapes. For such a field `clap_derive` builds
//! the argument id from `ident.unraw().to_string()` and kebab-cases only the
//! *long flag*, so clap records the id `type` and renders the flag `--type`.
//!
//! The `OrthoConfig` derive has to agree with that on three paths, and all
//! three are load-bearing:
//!
//! - `CliValueExtractor` queries `ArgMatches::value_source(#arg_id)` for every
//!   `cli_default_as_absent` field. An id of `r#type` never matches the
//!   argument clap recorded as `type`, so the lookup reports `None` for a flag
//!   the user actually supplied and the command-line value is silently
//!   dropped in favour of the file layer.
//! - The generated CLI struct declares its own `#[arg(long = ...)]`, so the
//!   derive computes the flag text itself. `heck` splits `r#type` at the `#`,
//!   giving `--r-type` unless the identifier is unrawed first.
//! - The serialized key feeds the generated file-key metadata, which rejects
//!   any segment outside ASCII alphanumeric plus `_` and `-`. A key of
//!   `r#type` therefore fails the derive outright, so the crate cannot compile
//!   at all rather than misreporting one field.
//!
//! All three are invisible without a test that parses a raw-identifier flag,
//! because the wrong id and the wrong flag each fail quietly at runtime while
//! the wrong key fails at compile time.

use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::{CliValueExtractor, OrthoConfig};
use serde::{Deserialize, Serialize};

#[path = "support/default_punct.rs"]
mod default_punct;

/// A clap-only raw-identifier field, with no `OrthoConfig` derive.
///
/// This fixture isolates upstream behaviour from anything this crate derives,
/// so the expected id and flag are pinned to clap rather than to a snapshot of
/// our own output.
#[derive(Debug, Parser)]
#[command(name = "raw-ident-upstream")]
struct UpstreamRawIdent {
    #[arg(long)]
    r#type: String,
}

/// A raw-identifier field carrying a clap default, through the full derive.
///
/// The user's own `#[arg]` writes the flag clap's `Parser` derive would give
/// this field, so the extractor is driven by the command a user would really
/// get. The generated CLI struct computes its *own* `long` from the field
/// name, which is the path [`generated_cli_renders_the_unrawed_flag`] covers.
///
/// The field deliberately carries no `#[serde(rename = "type")]`. Serde's own
/// unrawing makes it serialize under `type` regardless, and leaving the
/// attribute off keeps the derive's computed key on the tested path: the
/// generated file-key metadata rejects a raw segment, so a key of `r#type`
/// would stop this file from compiling rather than fail an assertion.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "raw-ident")]
#[ortho_config(prefix = "APP_")]
struct RawIdentArgs {
    /// Punctuation "type" is a Rust keyword, so the field is written raw.
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
fn clap_records_the_unrawed_id_and_flag() {
    let mut command = UpstreamRawIdent::command();
    let ids = arg_ids(&command);
    let help = command.render_long_help().to_string();

    ensure_contains(&ids, "type");
    assert!(
        !ids.iter().any(|id| id.starts_with("r#")),
        "clap must not record the raw spelling; got {ids:?}",
    );
    assert!(
        help.contains("--type"),
        "clap must derive the unrawed long flag; help was:\n{help}",
    );
}

/// The generated CLI struct records the id clap would record.
///
/// The generated struct derives its own `clap::Parser`, so its ids come from
/// our attribute rather than from a handwritten one. This is the assertion
/// that fails if the resolver ever reverts to `Ident::to_string()`.
#[test]
fn generated_cli_records_the_unrawed_id() {
    let ids = arg_ids(&__RawIdentArgsCli::command());

    ensure_contains(&ids, "type");
    assert!(
        !ids.iter().any(|id| id.starts_with("r#")),
        "the generated CLI must not record the raw spelling; got {ids:?}",
    );
}

/// The generated CLI struct renders the flag clap would render.
///
/// `heck` kebab-cases by splitting on non-alphanumeric characters, so `r#type`
/// becomes `r-type` unless the identifier is unrawed first. Asserting the
/// rendered help rather than the computed string keeps the check on what a
/// user actually sees.
#[test]
fn generated_cli_renders_the_unrawed_flag() {
    let mut command = __RawIdentArgsCli::command();
    let help = command.render_long_help().to_string();

    assert!(
        help.contains("--type"),
        "the generated CLI must render the unrawed flag; help was:\n{help}",
    );
    assert!(
        !help.contains("--r-type"),
        "the generated CLI must not leak the raw spelling; help was:\n{help}",
    );
}

/// An explicit raw-identifier flag is attributed to the command line.
///
/// This is the derive/runtime agreement assertion: `--type` is parsed by clap
/// under the id `type`, and the generated extractor finds it there. A resolver
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
/// a clap default. This case matters as much as the one above: an extractor
/// that matched *nothing at all* would also report the field absent, so the
/// pair is what separates "found the caller's value" from "found nothing".
#[test]
fn absent_raw_identifier_flag_leaves_the_clap_default_out() -> Result<()> {
    let extracted = parse_and_extract(["raw-ident"])?;

    ensure!(
        extracted.get("type").is_none(),
        "clap's default must not be attributed to the caller, got: {extracted:?}",
    );

    Ok(())
}

/// Parses `cli_args` with the user-visible command and extracts the
/// caller-supplied fields.
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

/// Collects the argument ids a command records, as owned strings.
fn arg_ids(command: &clap::Command) -> Vec<String> {
    command
        .get_arguments()
        .map(|arg| arg.get_id().to_string())
        .collect()
}

/// Asserts `ids` contains `expected`, naming the observed ids on failure.
fn ensure_contains(ids: &[String], expected: &str) {
    assert!(
        ids.iter().any(|id| id == expected),
        "expected arg id {expected:?}; got {ids:?}",
    );
}
