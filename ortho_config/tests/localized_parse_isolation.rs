//! Localized `clap` diagnostics for a missing required positional.
//!
//! These tests pin the exact first line of the localized error, so the
//! isolation marks `Fluent` inserts by default are visible in the assertion
//! rather than hidden behind a stripped comparison. They also prove the
//! builder's opt-out reaches the built-in Clap error copy, which is what a
//! consumer sees when a required positional is omitted.

use clap::{ColorChoice, CommandFactory, Parser};
use ortho_config::{
    FluentLocalizer, FluentLocalizerError, LocalizeCmd, Localizer, NoOpLocalizer, langid,
    parse_localized_command,
};

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

/// Builds the embedded English localizer, opting out of isolation when asked.
///
/// Returns the build error unchanged so each test decides how to fail.
fn english_localizer(use_isolating: bool) -> Result<FluentLocalizer, FluentLocalizerError> {
    FluentLocalizer::builder(langid!("en-US"))
        .with_use_isolating(use_isolating)
        .try_build()
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

#[test]
fn localized_missing_positional_keeps_isolation_marks_by_default() {
    let localizer = english_localizer(true).expect("embedded English catalogue should build");
    let error = missing_positional_error(&localizer).expect("the required positional is missing");

    let expected = format!(
        "error: Missing required argument: {FIRST_STRONG_ISOLATE}<PATH>{POP_DIRECTIONAL_ISOLATE}"
    );

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument,
        "a missing positional should report a missing required argument"
    );
    assert_eq!(
        first_line(&error).as_deref(),
        Some(expected.as_str()),
        "the default setting should keep Fluent's isolation marks around the argument"
    );
}

#[test]
fn localized_missing_positional_drops_isolation_marks_when_disabled() {
    let localizer = english_localizer(false).expect("embedded English catalogue should build");
    let error = missing_positional_error(&localizer).expect("the required positional is missing");

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument,
        "a missing positional should report a missing required argument"
    );
    assert_eq!(
        first_line(&error).as_deref(),
        Some("error: Missing required argument: <PATH>"),
        "disabling isolation should leave the argument free of control characters"
    );
}

#[test]
fn disabling_isolation_keeps_the_rendered_usage_tail() {
    let localizer = english_localizer(false).expect("embedded English catalogue should build");
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
    let isolating = english_localizer(true).expect("embedded English catalogue should build");
    let plain = english_localizer(false).expect("embedded English catalogue should build");
    let with_marks = missing_positional_error(&isolating)
        .expect("the required positional is missing")
        .to_string();
    let without_marks = missing_positional_error(&plain)
        .expect("the required positional is missing")
        .to_string();

    assert!(
        with_marks.contains(FIRST_STRONG_ISOLATE) && with_marks.contains(POP_DIRECTIONAL_ISOLATE),
        "the default output should carry both isolation marks: {with_marks:?}"
    );
    assert!(
        !without_marks.contains(FIRST_STRONG_ISOLATE)
            && !without_marks.contains(POP_DIRECTIONAL_ISOLATE),
        "the opted-out output should carry neither isolation mark: {without_marks:?}"
    );
}
