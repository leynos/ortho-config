//! Unit tests for the flag and placeholder formatters.
//!
//! These pin the exact roff each flag form renders, including the encoded
//! form of a hostile flag name.

use super::*;
use crate::schema::{CliMetadata, ValueType};
use rstest::rstest;

use super::super::tests::starts_a_request_line;

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
    assert_eq!(
        format_flag(long, short),
        expected,
        "flag {long:?}/{short:?} should render as {expected:?}"
    );
}

#[rstest]
#[case(Some("x\n.tm injected"), None)]
#[case(Some("x\r.so /etc/passwd"), None)]
#[case(None, Some('"'))]
fn flag_names_cannot_start_a_request_line(#[case] long: Option<&str>, #[case] short: Option<char>) {
    let rendered = format_flag(long, short);
    assert!(
        !starts_a_request_line(&rendered),
        "flag name rendered a request line: {rendered:?}"
    );
    assert!(
        !rendered.contains(['\n', '\r']),
        "flag name should not span lines: {rendered:?}"
    );
}

#[rstest]
#[case(
    Some("port"),
    Some('p'),
    "\\fB\\-\\-port\\fR \\fINUM\\fR, \\fB\\-p\\fR \\fINUM\\fR"
)]
#[case(Some("port"), None, "\\fB\\-\\-port\\fR \\fINUM\\fR")]
#[case(None, Some('p'), "\\fB\\-p\\fR \\fINUM\\fR")]
#[case(None, None, "\\fINUM\\fR")]
fn required_value_placeholder_follows_the_flag(
    #[case] long: Option<&str>,
    #[case] short: Option<char>,
    #[case] expected: &str,
) {
    // The required form separates the flag from its placeholder with a space,
    // which is what distinguishes it from the optional form below. When neither
    // flag is present the separator has no flag to follow, so the bare
    // placeholder is returned alone.
    assert_eq!(
        format_flag_with_value(long, short, "NUM"),
        expected,
        "required value for {long:?}/{short:?} should render as {expected:?}"
    );
}

#[rstest]
fn flag_names_with_values_cannot_start_a_request_line() {
    let rendered = format_flag_with_value(Some("x\n.tm injected"), None, "NUM");
    assert!(
        !starts_a_request_line(&rendered),
        "flag name with a value rendered a request line: {rendered:?}"
    );
}

#[rstest]
#[case(
    Some("is-excited"),
    Some('i'),
    "\\fB\\-\\-is-excited\\fR\\fI[=BOOL]\\fR, \\fB\\-i\\fR\\fI[=BOOL]\\fR"
)]
#[case(Some("is-excited"), None, "\\fB\\-\\-is-excited\\fR\\fI[=BOOL]\\fR")]
#[case(None, Some('E'), "\\fB\\-E\\fR\\fI[=BOOL]\\fR")]
#[case(None, None, "\\fI[=BOOL]\\fR")]
fn optional_value_placeholder_joins_the_flag(
    #[case] long: Option<&str>,
    #[case] short: Option<char>,
    #[case] expected: &str,
) {
    // A space between the flag and the placeholder would document the
    // invalid `--flag =BOOL` spelling, which `require_equals` rejects.
    assert_eq!(
        format_flag_with_optional_value(long, short, "BOOL"),
        expected,
        "optional value for {long:?}/{short:?} should render as {expected:?}"
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
    assert_eq!(
        value_type_placeholder(&vt).as_ref(),
        expected,
        "value type {vt:?} should map to {expected:?}"
    );
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
    assert_eq!(
        placeholder.as_ref(),
        "",
        "a boolean has no natural placeholder"
    );
    assert_eq!(
        format_option(&bool_flag(None), Some(&placeholder), "VALUE"),
        "\\fB\\-\\-excited\\fR\\fI[=VALUE]\\fR",
        "an absent placeholder should fall back to VALUE"
    );
}

#[test]
fn explicit_empty_value_name_renders_the_fallback_placeholder() {
    let placeholder = value_type_placeholder(&ValueType::Bool);
    assert_eq!(
        format_option(&bool_flag(Some("")), Some(&placeholder), "VALUE"),
        "\\fB\\-\\-excited\\fR\\fI[=VALUE]\\fR",
        "an empty value name should fall back to VALUE"
    );
}

#[test]
fn non_empty_value_name_still_wins_over_the_fallback() {
    // Negative control: the filter must not discard a real placeholder.
    assert_eq!(
        format_option(&bool_flag(Some("BOOL")), Some("VALUE"), "FALLBACK"),
        "\\fB\\-\\-excited\\fR\\fI[=BOOL]\\fR",
        "a real value name should beat the fallback"
    );
}
