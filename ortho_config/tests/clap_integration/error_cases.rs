//! Error handling scenarios for CLI parsing.

use super::common::{
    OptionConfig, OrthoConfig, OrthoError, RequiredConfig, TestConfig, assert_ortho_error,
};
use anyhow::Result;
use ortho_config::MapEnv;
use rstest::rstest;
use std::sync::Arc;

#[rstest]
#[case::unknown_flag(&["prog", "--bogus"])]
#[case::duplicate_flag(&["prog", "--sample-value", "foo", "--sample-value", "bar"])]
fn rejects_cli_parsing_errors(#[case] args: &[&str]) {
    assert_ortho_error(
        TestConfig::load_from_iter(args.iter().copied()),
        "CLI parsing",
        |err| matches!(err, OrthoError::CliParsing(_)),
    )
    .expect("CLI parsing should produce the expected error variant");
}

/// The spellings the optional-value boolean contract rejects.
///
/// The generated argument sets `require_equals`, so only the attached form
/// carries a value. `--is-excited false` must therefore fail rather than read
/// `false` from the following token — that rule is what keeps a positional
/// operand from being swallowed as a boolean. `-ifalse` attaches a value to the
/// short flag without the required `=`, and `=invalid` reaches clap's value
/// parser, which rejects it instead of coercing anything to `false`.
#[rstest]
#[case::space_separated_value(&["prog", "--is-excited", "false"])]
#[case::short_without_equals(&["prog", "-ifalse"])]
#[case::invalid_value(&["prog", "--is-excited=invalid"])]
fn rejects_invalid_boolean_spellings(#[case] args: &[&str]) {
    assert_ortho_error(
        TestConfig::load_from_iter(args.iter().copied()),
        "CLI parsing",
        |err| matches!(err, OrthoError::CliParsing(_)),
    )
    .expect("an invalid boolean spelling should produce a CLI parsing error");
}

#[rstest]
fn option_field_rejects_invalid_value() {
    assert_ortho_error(
        OptionConfig::load_from_iter(["prog", "--maybe", "notanumber"]),
        "CLI parsing",
        |err| matches!(err, OrthoError::CliParsing(_)),
    )
    .expect("invalid option value should produce a CLI parsing error");
}

/// A required field with no value in any supplied source surfaces as an
/// `OrthoError::Merge` rather than a CLI parsing error.
#[rstest]
fn missing_required_field_surfaces_merge_error() -> Result<()> {
    let source = Arc::new(MapEnv::new());
    assert_ortho_error(
        RequiredConfig::load_from_iter_with_sources(["prog"], source.clone(), source),
        "merge",
        |err| matches!(err, OrthoError::Merge { .. }),
    )
}
