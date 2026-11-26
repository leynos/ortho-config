//! Regression coverage for Unicode isolation marks in localized clap errors.
//!
//! `FluentLocalizer` follows Fluent's default of wrapping interpolated values
//! in U+2068 FIRST STRONG ISOLATE and U+2069 POP DIRECTIONAL ISOLATE. These
//! tests pin the exact diagnostic clap 4.6.7 renders for a missing required
//! positional, both with the default setting and with isolation disabled.

use clap::error::ErrorKind;
use clap::{ColorChoice, CommandFactory, Parser};
use ortho_config::{FluentLocalizer, LocalizeCmd, langid, parse_localized_command};

/// Parser whose single positional argument is deliberately required.
#[derive(Debug, Parser)]
#[command(name = "isolation-fixture", bin_name = "isolation-fixture")]
struct RequiredPositional {
    /// File to read.
    path: String,
}

/// Parses an empty argument list, which must fail on the missing positional.
fn parse_without_positional(localizer: &FluentLocalizer) -> clap::Error {
    let command = RequiredPositional::command()
        .color(ColorChoice::Never)
        .localize(localizer);
    parse_localized_command::<RequiredPositional, _, _>(command, ["isolation-fixture"], localizer)
        .expect_err("the required positional argument is missing")
}

/// Returns the first rendered line, which is the translated message.
fn first_line(rendered: &str) -> &str {
    rendered.lines().next().unwrap_or_default()
}

#[test]
fn default_diagnostic_isolates_the_interpolated_argument() {
    let localizer = FluentLocalizer::embedded(langid!("en-US")).expect("embedded en-US catalogue");
    let error = parse_without_positional(&localizer);
    assert_eq!(
        error.kind(),
        ErrorKind::MissingRequiredArgument,
        "the fixture should fail on its missing positional"
    );
    let rendered = error.to_string();
    assert_eq!(
        first_line(&rendered),
        "error: Missing required argument: \u{2068}<PATH>\u{2069}",
        "the default diagnostic should keep Fluent's isolate marks"
    );
    assert!(
        rendered.contains("Usage: isolation-fixture <PATH>"),
        "the retained clap tail should keep the usage line, got: {rendered}"
    );
}

#[test]
fn disabled_isolation_produces_a_greppable_diagnostic() {
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .with_use_isolating(false)
        .try_build()
        .expect("embedded en-US catalogue");
    let error = parse_without_positional(&localizer);
    assert_eq!(
        error.kind(),
        ErrorKind::MissingRequiredArgument,
        "the fixture should fail on its missing positional"
    );
    let rendered = error.to_string();
    assert_eq!(
        first_line(&rendered),
        "error: Missing required argument: <PATH>",
        "disabling isolation should remove both isolate marks"
    );
    assert!(
        !rendered.contains('\u{2068}') && !rendered.contains('\u{2069}'),
        "no isolate mark should survive anywhere in the diagnostic, got: {rendered}"
    );
    assert!(
        rendered.contains("Usage: isolation-fixture <PATH>"),
        "the retained clap tail should keep the usage line, got: {rendered}"
    );
}
