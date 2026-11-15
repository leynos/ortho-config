//! Real-parse and snapshot coverage for the clap error translation matrix.
//!
//! Cargo reaches this module only through `clap_error_translations.rs`, which
//! keeps the target's name and that file within the 400-line limit.

use super::{FormattingIssues, reporting_localizer};
use clap::{
    Arg, ArgAction, ColorChoice, Command,
    error::{Error as ClapError, ErrorKind},
    value_parser,
};
use ortho_config::{FluentLocalizer, NoOpLocalizer, langid, localize_clap_error};
use rstest::rstest;
use std::fmt::Write as _;
use std::sync::Arc;

/// Builds one representative real parse error for each reachable kind.
///
/// Colour is disabled so the rendered text is deterministic for snapshots and
/// comparisons. `Io` and `Format` have no argv reproducer and are constructed
/// with [`ClapError::raw`] by the callers that need them.
fn parse_error(kind: ErrorKind) -> ClapError {
    let (command, argv) = reproducer(kind);
    match command.try_get_matches_from(argv) {
        Err(error) => error,
        Ok(_) => panic!("the synthetic argv for {kind:?} should be rejected by clap"),
    }
}

/// The command and argv that reproduce `kind`.
fn reproducer(kind: ErrorKind) -> (Command, &'static [&'static str]) {
    let Some(reproducer) = value_reproducer(kind).or_else(|| structure_reproducer(kind)) else {
        panic!("no parser reproducer for {kind:?}");
    };
    reproducer
}

/// A colour-disabled command with the fixture program name.
fn base_command() -> Command {
    Command::new("prog").color(ColorChoice::Never)
}

/// Reproducers for failures in a value's shape, arity, or separator.
fn value_reproducer(kind: ErrorKind) -> Option<(Command, &'static [&'static str])> {
    let reproducer = match kind {
        ErrorKind::InvalidValue => (
            base_command().arg(Arg::new("speed").value_parser(["fast", "slow"])),
            &["prog", "other"][..],
        ),
        ErrorKind::NoEquals => (
            base_command().arg(
                Arg::new("color")
                    .long("color")
                    .action(ArgAction::Set)
                    .require_equals(true),
            ),
            &["prog", "--color", "red"][..],
        ),
        ErrorKind::ValueValidation => (
            base_command().arg(Arg::new("num").value_parser(value_parser!(u8))),
            &["prog", "NotANumber"][..],
        ),
        ErrorKind::TooManyValues => (
            base_command().arg(Arg::new("arg").num_args(1..=2)),
            &["prog", "too", "many", "values"][..],
        ),
        ErrorKind::TooFewValues => (
            base_command().arg(Arg::new("some_opt").long("opt").num_args(3..)),
            &["prog", "--opt", "too", "few"][..],
        ),
        ErrorKind::WrongNumberOfValues => (
            base_command().arg(
                Arg::new("some_opt")
                    .long("opt")
                    .action(ArgAction::Set)
                    .num_args(2),
            ),
            &["prog", "--opt", "wrong"][..],
        ),
        _ => return None,
    };
    Some(reproducer)
}

/// Reproducers for failures in the argument or subcommand structure.
fn structure_reproducer(kind: ErrorKind) -> Option<(Command, &'static [&'static str])> {
    let reproducer = match kind {
        ErrorKind::UnknownArgument => (
            base_command().arg(Arg::new("flag").long("flag").action(ArgAction::SetTrue)),
            &["prog", "--other"][..],
        ),
        ErrorKind::InvalidSubcommand => (
            base_command().subcommand(Command::new("config").about("Used for configuration")),
            &["prog", "confi"][..],
        ),
        ErrorKind::ArgumentConflict => (
            base_command()
                .arg(
                    Arg::new("debug")
                        .long("debug")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("color"),
                )
                .arg(Arg::new("color").long("color").action(ArgAction::SetTrue)),
            &["prog", "--debug", "--color"][..],
        ),
        ErrorKind::MissingRequiredArgument => (
            base_command().arg(Arg::new("debug").required(true)),
            &["prog"][..],
        ),
        ErrorKind::MissingSubcommand => (
            base_command()
                .subcommand_required(true)
                .subcommand(Command::new("test")),
            &["prog"][..],
        ),
        _ => return None,
    };
    Some(reproducer)
}

/// Builds one stock error per translated kind for snapshot rendering.
///
/// `InvalidUtf8` is represented by a raw error so the snapshot stays identical
/// on every platform; its real unix parse path is covered separately.
fn translated_stock_errors() -> Vec<ClapError> {
    let parser_kinds = [
        ErrorKind::InvalidValue,
        ErrorKind::UnknownArgument,
        ErrorKind::InvalidSubcommand,
        ErrorKind::NoEquals,
        ErrorKind::ValueValidation,
        ErrorKind::TooManyValues,
        ErrorKind::TooFewValues,
        ErrorKind::WrongNumberOfValues,
        ErrorKind::ArgumentConflict,
        ErrorKind::MissingRequiredArgument,
        ErrorKind::MissingSubcommand,
    ];

    let mut errors: Vec<ClapError> = parser_kinds.into_iter().map(parse_error).collect();
    errors.push(ClapError::raw(
        ErrorKind::InvalidUtf8,
        "invalid UTF-8 was detected in one or more arguments",
    ));
    errors.push(ClapError::raw(ErrorKind::Io, "i/o failure"));
    errors.push(ClapError::raw(ErrorKind::Format, "format failure"));
    errors
}

/// Asserts one error translates to its embedded message and keeps its tail.
fn assert_translated_with_embedded_message(error: ClapError, fragment: &str) {
    let kind = error.kind();
    let issues = Arc::new(FormattingIssues::new());
    let localizer = reporting_localizer(&issues);
    let stock = error.to_string();

    let rendered = localize_clap_error(error, &localizer).to_string();

    assert!(
        rendered.contains(fragment),
        "{kind:?} should render {fragment:?}, got: {rendered}"
    );
    assert_ne!(
        rendered, stock,
        "{kind:?} should not keep the stock message"
    );
    assert!(
        !rendered.contains('{'),
        "{kind:?} left a placeholder unresolved: {rendered}"
    );
    {
        let failures = issues.ids();
        assert!(
            failures.is_empty(),
            "{kind:?} formatting issues: {failures:?}"
        );
    }
    if stock.contains("Usage:") {
        assert!(
            rendered.contains("Usage:"),
            "{kind:?} lost the rendered usage tail: {rendered}"
        );
    }
}

/// Translates one reachable parser error per case.
#[rstest]
#[case::invalid_value(ErrorKind::InvalidValue, "Invalid value for")]
#[case::unknown_argument(ErrorKind::UnknownArgument, "Unknown argument:")]
#[case::invalid_subcommand(ErrorKind::InvalidSubcommand, "Unknown subcommand:")]
#[case::no_equals(ErrorKind::NoEquals, "Equal sign is required")]
#[case::value_validation(ErrorKind::ValueValidation, "was rejected by the value parser")]
#[case::too_many_values(ErrorKind::TooManyValues, "Unexpected value for")]
#[case::too_few_values(ErrorKind::TooFewValues, "Too few values for")]
#[case::wrong_number_of_values(ErrorKind::WrongNumberOfValues, "Wrong number of values for")]
#[case::argument_conflict(ErrorKind::ArgumentConflict, "cannot be used with another argument")]
#[case::missing_required_argument(ErrorKind::MissingRequiredArgument, "Missing required argument:")]
#[case::missing_subcommand(ErrorKind::MissingSubcommand, "Missing subcommand.")]
fn reachable_parser_errors_render_the_embedded_translation(
    #[case] kind: ErrorKind,
    #[case] fragment: &str,
) {
    assert_translated_with_embedded_message(parse_error(kind), fragment);
}

/// Translates a real invalid-UTF-8 parse error.
#[cfg(unix)]
#[test]
fn invalid_utf8_parser_error_renders_the_embedded_translation() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let error = Command::new("prog")
        .color(ColorChoice::Never)
        .arg(Arg::new("utf8").short('u').action(ArgAction::Set))
        .try_get_matches_from(vec![
            OsString::from("prog"),
            OsString::from("-u"),
            OsString::from_vec(vec![0xE9]),
        ])
        .expect_err("the synthetic argv should be rejected by clap");
    assert_eq!(error.kind(), ErrorKind::InvalidUtf8);

    assert_translated_with_embedded_message(error, "The provided value is not valid UTF-8");
}

/// Translates the context-free kinds clap only builds from raw errors.
#[rstest]
#[case::io(ErrorKind::Io, "An input/output error occurred")]
#[case::format(ErrorKind::Format, "The command line could not be rendered")]
fn context_free_raw_errors_render_the_embedded_translation(
    #[case] kind: ErrorKind,
    #[case] fragment: &str,
) {
    assert_translated_with_embedded_message(ClapError::raw(kind, "stock body"), fragment);
}

/// Asserts no translation is attempted when the localizer declines.
#[test]
fn no_op_localizer_preserves_the_stock_diagnostic() {
    for error in translated_stock_errors() {
        let kind = error.kind();
        let stock = error.to_string();
        let rendered = localize_clap_error(error, &NoOpLocalizer::new()).to_string();

        assert_eq!(
            rendered, stock,
            "{kind:?} should keep the stock text with NoOpLocalizer"
        );
    }
}

/// Pins the finite, colour-disabled rendered translation matrix.
#[test]
fn rendered_translation_matrix_matches_the_snapshot() {
    let localizer =
        FluentLocalizer::embedded(langid!("en-US")).expect("embedded en-US catalogue should parse");

    let mut rendered = String::new();
    for error in translated_stock_errors() {
        let kind = error.kind();
        let translated = localize_clap_error(error, &localizer).to_string();
        write!(rendered, "--- {kind:?}\n{translated}\n")
            .expect("writing into a String cannot fail");
    }

    insta::assert_snapshot!("clap_error_translation_matrix", rendered);
}
