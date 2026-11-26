//! Localized `clap` diagnostics for a missing required positional.
//!
//! These tests pin the exact first line of the localized error, so the
//! isolation marks `Fluent` inserts by default are visible in the assertion
//! rather than hidden behind a stripped comparison. They also prove the
//! builder's opt-out reaches the built-in Clap error copy, which is what a
//! consumer sees when a required positional is omitted.
//!
//! Setup is inline in each test rather than in a fallible helper: the
//! workspace allows `expect` inside test bodies and denies assertions in
//! functions returning `Result`, so a helper returning `Result` and a test
//! asserting cannot be combined.

use clap::{ColorChoice, CommandFactory, Parser};
use ortho_config::{
    FluentLocalizer, LocalizeCmd, Localizer, NoOpLocalizer, langid, parse_localized_command,
};
use rstest::rstest;

/// U+2068 FIRST STRONG ISOLATE, which `Fluent` writes before an interpolated value.
const FIRST_STRONG_ISOLATE: char = '\u{2068}';
/// U+2069 POP DIRECTIONAL ISOLATE, which `Fluent` writes after an interpolated value.
const POP_DIRECTIONAL_ISOLATE: char = '\u{2069}';

#[derive(Debug, Parser)]
#[command(name = "proposal", bin_name = "proposal", color = ColorChoice::Never)]
struct ProposalCli {
    /// Proposal file to validate.
    path: String,
}

/// Builds the embedded English localizer with the requested isolation setting.
///
/// Returns the build error unchanged so each test decides how to fail.
fn english_localizer(
    use_isolating: Option<bool>,
) -> Result<FluentLocalizer, ortho_config::FluentLocalizerError> {
    let builder = FluentLocalizer::builder(langid!("en-US"));
    match use_isolating {
        Some(value) => builder.with_use_isolating(value).try_build(),
        None => builder.try_build(),
    }
}

/// Parses a command with no positional argument and returns the localized error.
///
/// Returns `None` when the command parsed, which would mean the fixture lost its
/// required positional.
fn missing_positional_error(localizer: &dyn Localizer) -> Option<clap::Error> {
    let command = ProposalCli::command().localize(&NoOpLocalizer::new());
    parse_localized_command::<ProposalCli, _, _>(command, ["proposal"], localizer).err()
}

/// Returns the first rendered line, which is the translated message.
///
/// Returns `None` for an error that renders no lines at all.
fn first_line(error: &clap::Error) -> Option<String> {
    error.to_string().lines().next().map(ToOwned::to_owned)
}

/// The exact first line a localized missing-positional error must render.
fn expected_first_line(use_isolating: bool) -> String {
    if use_isolating {
        format!(
            "error: Missing required argument: {FIRST_STRONG_ISOLATE}<PATH>{POP_DIRECTIONAL_ISOLATE}"
        )
    } else {
        String::from("error: Missing required argument: <PATH>")
    }
}

#[rstest]
#[case::unspecified(None, true)]
#[case::explicit_true(Some(true), true)]
#[case::explicit_false(Some(false), false)]
fn localized_missing_positional_first_line_matches_the_setting(
    #[case] setting: Option<bool>,
    #[case] is_isolating: bool,
) {
    let localizer = english_localizer(setting).expect("embedded English catalogue should build");
    let error = missing_positional_error(&localizer).expect("the required positional is missing");

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument,
        "a missing positional should report a missing required argument"
    );
    assert_eq!(
        first_line(&error).as_deref(),
        Some(expected_first_line(is_isolating).as_str()),
        "the first line should reflect the isolation setting {setting:?}"
    );
}

#[test]
fn disabling_isolation_keeps_the_rendered_usage_tail() {
    let localizer =
        english_localizer(Some(false)).expect("embedded English catalogue should build");
    let rendered = missing_positional_error(&localizer)
        .expect("the required positional is missing")
        .to_string();

    assert!(
        rendered.contains("Usage:"),
        "the retained clap tail should still describe usage: {rendered:?}"
    );
}

#[test]
fn disabling_isolation_differs_from_the_default_only_in_the_message_line() {
    let isolating = english_localizer(None).expect("embedded English catalogue should build");
    let plain = english_localizer(Some(false)).expect("embedded English catalogue should build");
    let with_marks = missing_positional_error(&isolating)
        .expect("the required positional is missing")
        .to_string();
    let without_marks = missing_positional_error(&plain)
        .expect("the required positional is missing")
        .to_string();

    assert!(
        with_marks.contains(FIRST_STRONG_ISOLATE) && with_marks.contains(POP_DIRECTIONAL_ISOLATE),
        "the unset default should carry both isolation marks: {with_marks:?}"
    );
    assert!(
        !without_marks.contains(FIRST_STRONG_ISOLATE)
            && !without_marks.contains(POP_DIRECTIONAL_ISOLATE),
        "the opted-out output should carry neither isolation mark: {without_marks:?}"
    );
}
