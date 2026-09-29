//! Generated `--help` coverage for boolean flags.
//!
//! The derive gives boolean fields an optional `=<BOOL>` value. This suite
//! renders the help clap actually produces for the generated parser and
//! asserts the placeholder, rather than asserting the emitted tokens: clap
//! derives a value placeholder from the *field name* when none is supplied, so
//! a token-level check would pass while `--help` still read
//! `--is-excited[=<IS_EXCITED>]`. Rendering the real command is the only way to
//! see which of the two clap used, and it is what a user reads.
//!
//! The struct is declared here rather than reused from `common` because the
//! derive emits its hidden parser struct with private visibility, beside the
//! configuration type and in the same module. A test in another module cannot
//! name it, and re-exporting it is rejected as a private-interface leak.

use anyhow::{Result, ensure};
use clap::{CommandFactory, Parser};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Configuration whose only fields are booleans.
///
/// Both the plain `bool` and the `Option<bool>` shape are covered: the issue
/// requires absent-versus-present semantics to survive for each.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELPTEST_")]
struct HelpConfig {
    /// A required-to-declare boolean that still has a default.
    #[ortho_config(default = false)]
    is_excited: bool,
    /// An optional boolean, which distinguishes absent from `false`.
    is_quiet: Option<bool>,
}

/// The hidden parser struct the derive generates for [`HelpConfig`].
///
/// The derive names it `__{Config}Cli` and attaches `clap::Parser`, so this is
/// the same argument definition the application parses with, not a handwritten
/// stand-in that could drift from it.
type GeneratedCli = __HelpConfigCli;

#[test]
fn boolean_flags_render_the_bool_placeholder() -> Result<()> {
    let help = GeneratedCli::command().render_help().to_string();

    for flag in ["--is-excited", "--is-quiet"] {
        let with_value = format!("{flag}[=<BOOL>]");
        ensure!(
            help.contains(&with_value),
            "expected `{with_value}` in generated help, got:\n{help}"
        );
    }
    Ok(())
}

/// Negative control for the assertion above.
///
/// The placeholder is `BOOL` only because the generated `#[arg(...)]` names it
/// explicitly; without that name clap derives one from the field. Asserting the
/// field-derived form is absent is what shows the explicit name is doing the
/// work rather than coincidentally matching, and it also pins the `--flag`
/// spelling as unchanged.
#[test]
fn boolean_flags_do_not_render_the_field_derived_placeholder() -> Result<()> {
    let help = GeneratedCli::command().render_help().to_string();

    for flag in ["--is-excited[=<IS_EXCITED>]", "--is-quiet[=<IS_QUIET>]"] {
        ensure!(
            !help.contains(flag),
            "expected the field-derived `{flag}` to be replaced by BOOL, got:\n{help}"
        );
    }
    Ok(())
}

/// The bare flag still parses as `true` while an explicit `=false` clears it.
///
/// This is the behaviour the placeholder documents, asserted through the same
/// generated parser the help is rendered from.
#[test]
fn generated_parser_round_trips_the_documented_spellings() -> Result<()> {
    let bare = GeneratedCli::try_parse_from(["prog", "--is-excited"])?;
    ensure!(
        bare.is_excited == Some(true),
        "expected bare flag to mean true, got {:?}",
        bare.is_excited
    );

    let cleared = GeneratedCli::try_parse_from(["prog", "--is-excited=false"])?;
    ensure!(
        cleared.is_excited == Some(false),
        "expected an explicit false, got {:?}",
        cleared.is_excited
    );

    let absent = GeneratedCli::try_parse_from(["prog"])?;
    ensure!(
        absent.is_excited.is_none(),
        "expected an omitted flag to stay absent, got {:?}",
        absent.is_excited
    );
    Ok(())
}
