//! Error handling scenarios for CLI parsing.

use super::common::{
    OptionConfig, OrthoError, RequiredConfig, TestConfig, assert_ortho_error, with_jail,
};
use anyhow::Result;
use rstest::rstest;

#[rstest]
#[case::unknown_flag(&["prog", "--bogus"])]
#[case::duplicate_flag(&["prog", "--sample-value", "foo", "--sample-value", "bar"])]
fn rejects_cli_parsing_errors(#[case] args: &[&str]) -> Result<()> {
    assert_ortho_error(
        TestConfig::load_from_iter(args.iter().copied()),
        "CLI parsing",
        |err| matches!(err, OrthoError::CliParsing(_)),
    )
}

#[rstest]
fn option_field_rejects_invalid_value() -> Result<()> {
    assert_ortho_error(
        OptionConfig::load_from_iter(["prog", "--maybe", "notanumber"]),
        "CLI parsing",
        |err| matches!(err, OrthoError::CliParsing(_)),
    )
}

#[rstest]
fn missing_required_field_surfaces_merge_error() -> Result<()> {
    with_jail(|_| {
        assert_ortho_error(RequiredConfig::load_from_iter(["prog"]), "merge", |err| {
            matches!(err, OrthoError::Merge { .. })
        })
    })
}
