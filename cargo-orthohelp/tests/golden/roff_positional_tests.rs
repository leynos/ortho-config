//! Golden tests for positional and greedy operand ordering in roff SYNOPSIS.
//!
//! These cases pin the ordering rules the synopsis renderer applies: ordinary
//! positional operands sort by clap index and precede named flags, while an
//! operand that consumes the rest of the command line follows every flag, and
//! a `last` operand also follows the `--` delimiter. Each fixture deliberately
//! differs from declaration order so a renderer that preserves it cannot
//! pass.

use cargo_orthohelp::ir::{LocalizedDocMetadata, LocalizedFieldMetadata};
use cargo_orthohelp::roff::{RoffConfig, generate_to_string};
use cargo_orthohelp::schema::{CliMetadata, PositionalMetadata, ValueType};
use rstest::rstest;

use super::minimal_metadata;

/// A positional operand's placement in a test field.
#[derive(Clone, Copy)]
struct Position {
    /// One-based clap index.
    index: u16,
    /// Whether the operand accepts more than one value.
    variadic: bool,
    /// Whether the operand is reachable only after `--`.
    last: bool,
    /// Whether the operand consumes the remaining command line.
    var_arg: bool,
}

impl Position {
    /// An ordinary indexed operand.
    const fn indexed(index: u16) -> Self {
        Self {
            index,
            variadic: false,
            last: false,
            var_arg: false,
        }
    }

    /// An operand that accepts repeated values.
    const fn variadic(index: u16) -> Self {
        Self {
            index,
            variadic: true,
            last: false,
            var_arg: false,
        }
    }

    /// An operand reachable only after the `--` delimiter.
    const fn last(index: u16) -> Self {
        Self {
            index,
            variadic: false,
            last: true,
            var_arg: false,
        }
    }

    /// An operand that consumes the remaining command line.
    const fn var_arg(index: u16) -> Self {
        Self {
            index,
            variadic: false,
            last: false,
            var_arg: true,
        }
    }
}

/// Builds a field carrying a value name, optionally as a positional operand.
fn test_field(
    name: &str,
    value_name: &str,
    required: bool,
    position: Option<Position>,
) -> LocalizedFieldMetadata {
    LocalizedFieldMetadata {
        name: name.to_owned(),
        help: format!("{name} field."),
        long_help: None,
        value: Some(ValueType::String),
        default: None,
        required,
        deprecated: None,
        cli: Some(CliMetadata {
            long: position.is_none().then(|| name.to_owned()),
            short: None,
            value_name: Some(value_name.to_owned()),
            multiple: false,
            takes_value: true,
            value_optional: false,
            possible_values: vec![],
            hide_in_help: false,
            positional: position.map(|placement| {
                PositionalMetadata::new(placement.index)
                    .with_variadic(placement.variadic)
                    .with_var_arg(placement.var_arg)
                    .with_last(placement.last)
            }),
        }),
        env: None,
        file: None,
        examples: vec![],
        links: vec![],
        notes: vec![],
    }
}

/// Finds one marker's offset in the rendered synopsis.
fn synopsis_offset(synopsis: &str, needle: &str) -> usize {
    let Some(offset) = synopsis.find(needle) else {
        panic!("the synopsis should contain {needle}: {synopsis}");
    };
    offset
}

/// Test that positionals are sorted by index and lead even when declared last.
///
/// The named flag is declared first and the operands are declared in reverse
/// index order, so a renderer that preserves declaration order fails both the
/// positional sort and the positional-before-flag reordering.
#[rstest]
fn golden_positional_operand_ordering(mut minimal_metadata: LocalizedDocMetadata) {
    minimal_metadata
        .fields
        .push(test_field("check", "COMMAND", false, None));
    minimal_metadata.fields.push(test_field(
        "second",
        "SECOND",
        true,
        Some(Position::indexed(2)),
    ));
    minimal_metadata.fields.push(test_field(
        "first",
        "FIRST",
        true,
        Some(Position::indexed(1)),
    ));

    let output = generate_to_string(&minimal_metadata, &RoffConfig::default());
    let synopsis = output
        .split(".SH DESCRIPTION")
        .next()
        .expect("synopsis should precede the description");

    let first_at = synopsis_offset(synopsis, "FIRST");
    let second_at = synopsis_offset(synopsis, "SECOND");
    let flag_at = synopsis_offset(synopsis, "\\-\\-check");
    assert!(
        first_at < second_at,
        "positionals should be ordered by clap index, not declaration order: {synopsis}"
    );
    assert!(
        second_at < flag_at,
        "ordinary positionals should lead the named flags: {synopsis}"
    );
    assert!(
        !synopsis.contains("\\-\\-first"),
        "the positional must not advertise a long flag: {synopsis}"
    );
}

/// Test that a `last` operand follows the named flags and the `--` delimiter.
///
/// clap treats everything after `--` as operand data, so a named flag placed
/// after the delimiter could never be parsed as a flag.
#[rstest]
fn golden_last_operand_follows_flags_and_delimiter(mut minimal_metadata: LocalizedDocMetadata) {
    minimal_metadata
        .fields
        .push(test_field("tail", "TAIL", true, Some(Position::last(1))));
    minimal_metadata
        .fields
        .push(test_field("mode", "MODE", false, None));

    let output = generate_to_string(&minimal_metadata, &RoffConfig::default());
    let synopsis = output
        .split(".SH DESCRIPTION")
        .next()
        .expect("synopsis should precede the description");

    let flag_at = synopsis_offset(synopsis, "\\-\\-mode");
    let delimiter_at = synopsis_offset(synopsis, "\\-\\-\n");
    let tail_at = synopsis_offset(synopsis, "TAIL");
    assert!(
        flag_at < delimiter_at,
        "named flags must precede the `--` delimiter: {synopsis}"
    );
    assert!(
        delimiter_at < tail_at,
        "a `last` operand must follow the `--` delimiter: {synopsis}"
    );
}

/// Test that an optional `last` operand keeps the delimiter optional too.
///
/// Emitting a bare `--` before a bracketed operand would claim the delimiter
/// is required even though a valid invocation may omit both.
#[rstest]
fn golden_optional_last_operand_groups_the_delimiter(mut minimal_metadata: LocalizedDocMetadata) {
    minimal_metadata
        .fields
        .push(test_field("mode", "MODE", false, None));
    minimal_metadata
        .fields
        .push(test_field("tail", "TAIL", false, Some(Position::last(1))));

    let output = generate_to_string(&minimal_metadata, &RoffConfig::default());
    let synopsis = output
        .split(".SH DESCRIPTION")
        .next()
        .expect("synopsis should precede the description");

    assert!(
        synopsis.contains("[\\-\\- \\fITAIL\\fR]"),
        "the optional delimiter and operand should share one group: {synopsis}"
    );
    assert!(
        !synopsis.contains("\\-\\-\n"),
        "an optional `last` operand must not advertise a mandatory delimiter: {synopsis}"
    );
}

/// Test that a variadic positional advertises repetition.
#[rstest]
fn golden_variadic_positional_shows_repetition(mut minimal_metadata: LocalizedDocMetadata) {
    minimal_metadata.fields.push(test_field(
        "paths",
        "PATH",
        true,
        Some(Position::variadic(1)),
    ));

    let output = generate_to_string(&minimal_metadata, &RoffConfig::default());
    let synopsis = output
        .split(".SH DESCRIPTION")
        .next()
        .expect("synopsis should precede the description");

    assert!(
        synopsis.contains("\\fIPATH\\fR..."),
        "a variadic operand should show repetition notation: {synopsis}"
    );
}

/// Test that a `trailing_var_arg` operand follows the named flags.
///
/// Once the operand starts, clap reads every later word as one of its values,
/// so a flag placed after it could never be parsed as a flag.
#[rstest]
fn golden_trailing_var_arg_follows_flags(mut minimal_metadata: LocalizedDocMetadata) {
    minimal_metadata.fields.push(test_field(
        "script",
        "SCRIPT",
        true,
        Some(Position::var_arg(1)),
    ));
    minimal_metadata
        .fields
        .push(test_field("mode", "MODE", false, None));

    let output = generate_to_string(&minimal_metadata, &RoffConfig::default());
    let synopsis = output
        .split(".SH DESCRIPTION")
        .next()
        .expect("synopsis should precede the description");

    let flag_at = synopsis_offset(synopsis, "\\-\\-mode");
    let script_at = synopsis_offset(synopsis, "SCRIPT");
    assert!(
        flag_at < script_at,
        "named flags must precede a trailing-var-arg operand: {synopsis}"
    );
    assert!(
        !synopsis.contains("\\-\\-\n"),
        "a trailing-var-arg operand does not require the `--` delimiter: {synopsis}"
    );
}
