//! End-to-end checks that translated errors keep the detail clap supplies.
//!
//! Two of `clap`'s diagnostics carry detail outside the plain context map: an
//! `ArgumentConflict` distinguishes a repeated flag from a collision between
//! distinct arguments, and a value parser's rejection reason lives on the
//! error's source. These cases run the workspace fixture so the surviving
//! detail is asserted in the process-visible text, where an implementation
//! that dropped it would still look like a plausible translation.

use std::process::Output;

use super::{run_fixture, stderr_of, stdout_of};

/// The character Fluent opens a bidi-isolated placeable with.
const ISOLATE_START: char = '\u{2068}';

/// The character Fluent closes a bidi-isolated placeable with.
const ISOLATE_END: char = '\u{2069}';

/// Asserts a usage error's status and stream, returning its readable text.
///
/// Fluent isolates every interpolated value so a right-to-left locale cannot
/// reorder it around the surrounding prose. The marks are invisible in a
/// terminal but would sit inside any assertion quoting a value, so the text is
/// returned without them and assertions can quote what a user reads.
fn usage_error_text(output: &Output) -> String {
    assert_eq!(
        output.status.code(),
        Some(2),
        "a usage error should keep clap's exit code, got: {:?}",
        output.status.code()
    );
    assert!(
        stdout_of(output).is_empty(),
        "a usage error should not write to stdout, got: {}",
        stdout_of(output)
    );
    stderr_of(output).replace(&[ISOLATE_START, ISOLATE_END][..], "")
}

/// A flag repeated on the command line is reported as a repeat.
///
/// `clap` raises `ArgumentConflict` for `--debug --debug` exactly as it does
/// for two arguments that conflict, so this case fails if the translation
/// blames a second argument that was never supplied.
#[test]
fn a_repeated_flag_is_reported_as_a_repeat() {
    let output = run_fixture("en-US", &["--debug", "--debug"]).expect("the fixture should run");
    let text = usage_error_text(&output);

    assert!(
        text.contains("cannot be used multiple times"),
        "expected the repeated-argument translation, got: {text}"
    );
    assert!(
        !text.contains("cannot be used with"),
        "a repeated flag conflicts with nothing else, got: {text}"
    );
    assert!(
        text.contains("--debug"),
        "expected the offending argument, got: {text}"
    );
    assert!(
        !text.contains('{'),
        "no placeholder should remain unresolved, got: {text}"
    );
}

/// Two distinct conflicting arguments are named, in both positions.
///
/// The translation mirrors `clap`'s own rendering, which reports the argument
/// that was already accepted and the one it collided with, so a message that
/// said "another argument" would discard detail the stock text provides.
#[test]
fn distinct_arguments_report_what_they_conflict_with() {
    let output = run_fixture("en-US", &["--debug", "--trace"]).expect("the fixture should run");
    let text = usage_error_text(&output);

    assert!(
        text.contains("cannot be used with --trace"),
        "expected the conflicting argument to be named, got: {text}"
    );
    assert!(
        text.contains("--debug"),
        "expected the argument already accepted to be named, got: {text}"
    );
    assert!(
        !text.contains("multiple times"),
        "distinct arguments are not a repeat, got: {text}"
    );
    assert!(
        !text.contains('{'),
        "no placeholder should remain unresolved, got: {text}"
    );
}

/// A value parser's rejection reason survives translation.
///
/// `clap` keeps the reason on the error's source rather than in its context
/// map, and renders it after the generic text. A translation that dropped it
/// would say a parser declined the value without saying why.
#[test]
fn a_value_parser_rejection_keeps_its_reason() {
    let output = run_fixture("en-US", &["--count", "abc"]).expect("the fixture should run");
    let text = usage_error_text(&output);

    assert!(
        text.contains("was rejected by the value parser"),
        "expected the value-validation translation, got: {text}"
    );
    assert!(
        text.contains("must be a positive whole number"),
        "expected the parser's own reason, got: {text}"
    );
    assert!(
        text.contains("abc"),
        "expected the rejected value, got: {text}"
    );
    assert!(
        text.contains("--count"),
        "expected the offending argument, got: {text}"
    );
    assert!(
        !text.contains('{'),
        "no placeholder should remain unresolved, got: {text}"
    );
}
