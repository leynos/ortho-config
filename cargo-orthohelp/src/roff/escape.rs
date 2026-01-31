//! Roff text escaping and formatting utilities.
//!
//! Provides functions for escaping text and formatting CLI options for safe
//! inclusion in roff man page output.

#![allow(
    clippy::integer_division,
    reason = "capacity heuristic, precision loss acceptable"
)]
#![allow(clippy::integer_division_remainder_used, reason = "capacity heuristic")]

use crate::schema::ValueType;

/// Escapes text for safe inclusion in roff output.
///
/// Handles:
/// - Backslashes: `\` -> `\\`
/// - Dashes at line start: `-` -> `\-` (prevents option interpretation)
/// - Periods at line start: `.` -> `\&.` (prevents macro interpretation)
/// - Single quotes at line start: `'` -> `\&'` (prevents macro interpretation)
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::escape_text;
///
/// assert_eq!(escape_text("hello"), "hello");
/// assert_eq!(escape_text("path\\to\\file"), "path\\\\to\\\\file");
/// assert_eq!(escape_text("-flag"), "\\-flag");
/// ```
#[must_use]
pub fn escape_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + text.len() / 8);

    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        escape_line(line, &mut result);
    }

    // Preserve trailing newline if present
    if text.ends_with('\n') {
        result.push('\n');
    }

    result
}

fn escape_line(line: &str, result: &mut String) {
    let mut chars = line.chars().peekable();

    // Handle leading special characters
    if let Some(&first) = chars.peek() {
        match first {
            '-' => {
                result.push_str("\\-");
                chars.next();
            }
            '.' => {
                result.push_str("\\&.");
                chars.next();
            }
            '\'' => {
                result.push_str("\\&'");
                chars.next();
            }
            _ => {}
        }
    }

    // Process remaining characters
    for ch in chars {
        match ch {
            '\\' => result.push_str("\\\\"),
            _ => result.push(ch),
        }
    }
}

/// Formats text as bold using inline font escapes.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::bold;
///
/// assert_eq!(bold("text"), "\\fBtext\\fR");
/// ```
#[must_use]
pub fn bold(text: &str) -> String {
    format!("\\fB{text}\\fR")
}

/// Formats text as italic using inline font escapes.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::italic;
///
/// assert_eq!(italic("text"), "\\fItext\\fR");
/// ```
#[must_use]
pub fn italic(text: &str) -> String {
    format!("\\fI{text}\\fR")
}

/// Formats a CLI flag with proper roff markup.
///
/// Returns bold formatted flags, combining long and short forms when both
/// are present.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::format_flag;
///
/// assert_eq!(format_flag(Some("verbose"), Some('v')), "\\fB\\-\\-verbose\\fR, \\fB\\-v\\fR");
/// assert_eq!(format_flag(Some("help"), None), "\\fB\\-\\-help\\fR");
/// assert_eq!(format_flag(None, Some('h')), "\\fB\\-h\\fR");
/// ```
#[must_use]
pub fn format_flag(long: Option<&str>, short: Option<char>) -> String {
    match (long, short) {
        (Some(l), Some(s)) => format!("\\fB\\-\\-{l}\\fR, \\fB\\-{s}\\fR"),
        (Some(l), None) => format!("\\fB\\-\\-{l}\\fR"),
        (None, Some(s)) => format!("\\fB\\-{s}\\fR"),
        (None, None) => String::new(),
    }
}

/// Formats a CLI flag with a value placeholder.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::format_flag_with_value;
///
/// assert_eq!(
///     format_flag_with_value(Some("port"), Some('p'), "NUM"),
///     "\\fB\\-\\-port\\fR \\fINUM\\fR, \\fB\\-p\\fR \\fINUM\\fR"
/// );
/// ```
#[must_use]
pub fn format_flag_with_value(long: Option<&str>, short: Option<char>, value_name: &str) -> String {
    let value = italic(value_name);
    match (long, short) {
        (Some(l), Some(s)) => format!("\\fB\\-\\-{l}\\fR {value}, \\fB\\-{s}\\fR {value}"),
        (Some(l), None) => format!("\\fB\\-\\-{l}\\fR {value}"),
        (None, Some(s)) => format!("\\fB\\-{s}\\fR {value}"),
        (None, None) => value,
    }
}

/// Returns a human-readable placeholder for a `ValueType`.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::value_type_placeholder;
/// use cargo_orthohelp::schema::ValueType;
///
/// assert_eq!(value_type_placeholder(&ValueType::String), "STRING");
/// assert_eq!(value_type_placeholder(&ValueType::Path), "PATH");
/// ```
#[must_use]
pub const fn value_type_placeholder(value_type: &ValueType) -> &'static str {
    match value_type {
        ValueType::String => "STRING",
        ValueType::Integer { .. } => "INT",
        ValueType::Float { .. } => "FLOAT",
        ValueType::Bool => "",
        ValueType::Duration => "DURATION",
        ValueType::Path => "PATH",
        ValueType::IpAddr => "IP",
        ValueType::Hostname => "HOST",
        ValueType::Url => "URL",
        ValueType::Enum { .. } => "CHOICE",
        ValueType::List { .. } => "LIST",
        ValueType::Map { .. } => "MAP",
        ValueType::Custom { .. } => "VALUE",
    }
}

#[cfg(test)]
mod tests {
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
    fn bold_wraps_text() {
        assert_eq!(bold("text"), "\\fBtext\\fR");
    }

    #[rstest]
    fn italic_wraps_text() {
        assert_eq!(italic("text"), "\\fItext\\fR");
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
    #[case(ValueType::Custom { name: "MyType".to_owned() }, "VALUE")]
    fn value_type_placeholder_mapping(#[case] vt: ValueType, #[case] expected: &str) {
        assert_eq!(value_type_placeholder(&vt), expected);
    }
}
