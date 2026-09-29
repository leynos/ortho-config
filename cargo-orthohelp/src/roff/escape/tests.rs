//! Unit tests for roff escaping.

use super::*;
use rstest::rstest;

#[rstest]
#[case("hello", "hello")]
#[case("path\\to\\file", "path\\\\to\\\\file")]
#[case("-flag", "\\-flag")]
#[case(".macro", "\\&.macro")]
#[case("'quote", "\\&'quote")]
#[case("normal-dash", "normal-dash")]
#[case("a.period", "a.period")]
fn escape_text_handles_special_chars(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(escape_text(input), expected);
}

#[rstest]
fn escape_text_handles_multiline() {
    let input = "-first\n.second\n'third";
    let expected = "\\-first\n\\&.second\n\\&'third";
    assert_eq!(escape_text(input), expected);
}

#[rstest]
fn escape_text_preserves_trailing_newline() {
    assert_eq!(escape_text("hello\n"), "hello\n");
    assert_eq!(escape_text("hello"), "hello");
}

#[rstest]
#[case("hello", "hello")]
#[case("path\\to\\file", "path\\\\to\\\\file")]
#[case("with \"quotes\"", "with \\(dqquotes\\(dq")]
#[case("mixed\\and\"both", "mixed\\\\and\\(dqboth")]
fn escape_macro_arg_handles_special_chars(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(escape_macro_arg(input), expected);
}

#[rstest]
fn bold_wraps_and_escapes_text() {
    assert_eq!(bold("text"), "\\fBtext\\fR");
    assert_eq!(bold("path\\to"), "\\fBpath\\\\to\\fR");
}

#[rstest]
fn italic_wraps_and_escapes_text() {
    assert_eq!(italic("text"), "\\fItext\\fR");
    assert_eq!(italic("path\\to"), "\\fIpath\\\\to\\fR");
}

#[rstest]
#[case(Some("verbose"), Some('v'), "\\fB\\-\\-verbose\\fR, \\fB\\-v\\fR")]
#[case(Some("help"), None, "\\fB\\-\\-help\\fR")]
#[case(None, Some('h'), "\\fB\\-h\\fR")]
#[case(None, None, "")]
fn format_flag_combinations(
    #[case] long: Option<&str>,
    #[case] short: Option<char>,
    #[case] expected: &str,
) {
    assert_eq!(format_flag(long, short), expected);
}

#[rstest]
#[case(
    Some("is-excited"),
    Some('i'),
    "\\fB\\-\\-is-excited\\fR\\fI[=BOOL]\\fR, \\fB\\-i\\fR\\fI[=BOOL]\\fR"
)]
#[case(Some("is-excited"), None, "\\fB\\-\\-is-excited\\fR\\fI[=BOOL]\\fR")]
#[case(None, Some('E'), "\\fB\\-E\\fR\\fI[=BOOL]\\fR")]
fn optional_value_placeholder_joins_the_flag(
    #[case] long: Option<&str>,
    #[case] short: Option<char>,
    #[case] expected: &str,
) {
    // A space between the flag and the placeholder would document the
    // invalid `--flag =BOOL` spelling, which `require_equals` rejects.
    assert_eq!(
        format_flag_with_optional_value(long, short, "BOOL"),
        expected
    );
}

#[rstest]
#[case(ValueType::String, "STRING")]
#[case(ValueType::Integer { bits: 32, signed: true }, "INT")]
#[case(ValueType::Float { bits: 64 }, "FLOAT")]
#[case(ValueType::Bool, "")]
#[case(ValueType::Duration, "DURATION")]
#[case(ValueType::Path, "PATH")]
#[case(ValueType::IpAddr, "IP")]
#[case(ValueType::Hostname, "HOST")]
#[case(ValueType::Url, "URL")]
#[case(ValueType::Enum { variants: vec![] }, "CHOICE")]
#[case(ValueType::List { of: Box::new(ValueType::String) }, "LIST")]
#[case(ValueType::Map { of: Box::new(ValueType::String) }, "MAP")]
#[case(ValueType::Custom { name: "MyType".to_owned() }, "MYTYPE")]
fn value_type_placeholder_mapping(#[case] vt: ValueType, #[case] expected: &str) {
    assert_eq!(value_type_placeholder(&vt).as_ref(), expected);
}

fn bool_flag(value_name: Option<&str>) -> CliMetadata {
    CliMetadata {
        long: Some("excited".to_owned()),
        short: None,
        value_name: value_name.map(str::to_owned),
        multiple: false,
        takes_value: true,
        value_optional: true,
        possible_values: vec!["true".to_owned(), "false".to_owned()],
        hide_in_help: false,
    }
}

#[test]
fn bool_flag_without_a_value_name_renders_the_fallback_placeholder() {
    // The roff call sites derive the placeholder from `ValueType::Bool`,
    // which yields `""`; passing that through unfiltered rendered the
    // malformed `--flag[=]` rather than a usable synopsis.
    let placeholder = value_type_placeholder(&ValueType::Bool);
    assert_eq!(placeholder.as_ref(), "");
    assert_eq!(
        format_option(&bool_flag(None), Some(&placeholder), "VALUE"),
        "\\fB\\-\\-excited\\fR\\fI[=VALUE]\\fR"
    );
}

#[test]
fn explicit_empty_value_name_renders_the_fallback_placeholder() {
    let placeholder = value_type_placeholder(&ValueType::Bool);
    assert_eq!(
        format_option(&bool_flag(Some("")), Some(&placeholder), "VALUE"),
        "\\fB\\-\\-excited\\fR\\fI[=VALUE]\\fR"
    );
}

#[test]
fn non_empty_value_name_still_wins_over_the_fallback() {
    // Negative control: the filter must not discard a real placeholder.
    assert_eq!(
        format_option(&bool_flag(Some("BOOL")), Some("VALUE"), "FALLBACK"),
        "\\fB\\-\\-excited\\fR\\fI[=BOOL]\\fR"
    );
}
