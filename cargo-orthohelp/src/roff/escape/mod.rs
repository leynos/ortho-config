//! Roff text escaping and formatting utilities.
//!
//! Provides functions for escaping text and formatting CLI options for safe
//! inclusion in roff man page output.

use std::borrow::Cow;

use crate::schema::{CliMetadata, ValueType};

/// Formats a CLI option, choosing the optional-value form for boolean flags.
///
/// Boolean options render as `--flag[=BOOL]`, with the bracketed placeholder
/// italicized; everything else renders as an ordinary flag with a required
/// value, or as a bare switch when [`CliMetadata::takes_value`] is false.
///
/// `fallback_placeholder` supplies a value name when the metadata does not
/// carry one, derived from the field's semantic [`ValueType`]. An empty
/// placeholder counts as absent: [`value_type_placeholder`] returns `""` for
/// [`ValueType::Bool`], and every roff call site passes that result in here, so
/// without the filter a boolean would render the malformed `--flag[=]` instead
/// of a readable `--flag[=VALUE]`.
#[must_use]
pub fn format_option(
    cli: &CliMetadata,
    fallback_placeholder: Option<&str>,
    fallback: &str,
) -> String {
    if !cli.takes_value {
        return format_flag(cli.long.as_deref(), cli.short);
    }
    let value_name = cli
        .value_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .or_else(|| fallback_placeholder.filter(|name| !name.is_empty()))
        .unwrap_or(fallback);
    if cli.value_optional {
        format_flag_with_optional_value(cli.long.as_deref(), cli.short, value_name)
    } else {
        format_flag_with_value(cli.long.as_deref(), cli.short, value_name)
    }
}

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
    let mut result = String::with_capacity(text.len() + text.len().div_ceil(8));

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
    let mut chars = line.chars();

    // Handle leading special character
    if let Some(first) = chars.next()
        && !push_escaped_leading_char(first, result)
    {
        push_escaped_char(first, result);
    }

    // Process remaining characters
    for ch in chars {
        push_escaped_char(ch, result);
    }
}

fn push_escaped_leading_char(ch: char, result: &mut String) -> bool {
    match ch {
        '-' => {
            result.push_str("\\-");
            true
        }
        '.' => {
            result.push_str("\\&.");
            true
        }
        '\'' => {
            result.push_str("\\&'");
            true
        }
        _ => false,
    }
}

fn push_escaped_char(ch: char, result: &mut String) {
    match ch {
        '\\' => result.push_str("\\\\"),
        _ => result.push(ch),
    }
}

/// Escapes text for inclusion in quoted roff macro arguments.
///
/// Handles:
/// - Backslashes: `\` -> `\\`
/// - Double quotes: `"` -> `\(dq`
///
/// This function is designed for use in macro arguments like `.TH "NAME" "1" "DATE"`.
#[must_use]
pub fn escape_macro_arg(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + text.len().div_ceil(8));
    for ch in text.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\(dq"),
            _ => result.push(ch),
        }
    }
    result
}

/// Formats text as bold using inline font escapes.
///
/// The text is escaped before formatting to prevent roff control character issues.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::bold;
///
/// assert_eq!(bold("text"), "\\fBtext\\fR");
/// assert_eq!(bold("path\\to"), "\\fBpath\\\\to\\fR");
/// ```
#[must_use]
pub fn bold(text: &str) -> String {
    let escaped = escape_text(text);
    format!("\\fB{escaped}\\fR")
}

/// Formats text as italic using inline font escapes.
///
/// The text is escaped before formatting to prevent roff control character issues.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::italic;
///
/// assert_eq!(italic("text"), "\\fItext\\fR");
/// assert_eq!(italic("path\\to"), "\\fIpath\\\\to\\fR");
/// ```
#[must_use]
pub fn italic(text: &str) -> String {
    let escaped = escape_text(text);
    format!("\\fI{escaped}\\fR")
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

/// Formats a CLI flag whose value is optional, as in `--flag[=BOOL]`.
///
/// Boolean options accept a value but do not require one: the bare spelling
/// means `true`, and `--flag=false` supplies an explicit `false`. Bracketing
/// the placeholder distinguishes the optional value from the required form
/// produced by [`format_flag_with_value`].
///
/// The placeholder is joined directly to the flag, with no intervening space,
/// because the value must follow an `=`. Rendering `--flag [=BOOL]` would
/// document the invalid `--flag =BOOL` spelling. This matches the suffix that
/// `clap` itself emits for an argument with `require_equals` set.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::format_flag_with_optional_value;
///
/// assert_eq!(
///     format_flag_with_optional_value(Some("is-excited"), Some('i'), "BOOL"),
///     "\\fB\\-\\-is-excited\\fR\\fI[=BOOL]\\fR, \\fB\\-i\\fR\\fI[=BOOL]\\fR"
/// );
/// ```
#[must_use]
pub fn format_flag_with_optional_value(
    long: Option<&str>,
    short: Option<char>,
    value_name: &str,
) -> String {
    let value = italic(&format!("[={value_name}]"));
    match (long, short) {
        (Some(l), Some(s)) => format!("\\fB\\-\\-{l}\\fR{value}, \\fB\\-{s}\\fR{value}"),
        (Some(l), None) => format!("\\fB\\-\\-{l}\\fR{value}"),
        (None, Some(s)) => format!("\\fB\\-{s}\\fR{value}"),
        (None, None) => value,
    }
}

/// Returns a human-readable placeholder for a `ValueType`.
///
/// For `Custom` types, returns the uppercased type name.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::value_type_placeholder;
/// use cargo_orthohelp::schema::ValueType;
///
/// assert_eq!(value_type_placeholder(&ValueType::String), "STRING");
/// assert_eq!(value_type_placeholder(&ValueType::Path), "PATH");
/// assert_eq!(
///     value_type_placeholder(&ValueType::Custom { name: "MyType".to_owned() }),
///     "MYTYPE"
/// );
/// ```
#[must_use]
pub fn value_type_placeholder(value_type: &ValueType) -> Cow<'static, str> {
    match value_type {
        ValueType::String => Cow::Borrowed("STRING"),
        ValueType::Integer { .. } => Cow::Borrowed("INT"),
        ValueType::Float { .. } => Cow::Borrowed("FLOAT"),
        ValueType::Bool => Cow::Borrowed(""),
        ValueType::Duration => Cow::Borrowed("DURATION"),
        ValueType::Path => Cow::Borrowed("PATH"),
        ValueType::IpAddr => Cow::Borrowed("IP"),
        ValueType::Hostname => Cow::Borrowed("HOST"),
        ValueType::Url => Cow::Borrowed("URL"),
        ValueType::Enum { .. } => Cow::Borrowed("CHOICE"),
        ValueType::List { .. } => Cow::Borrowed("LIST"),
        ValueType::Map { .. } => Cow::Borrowed("MAP"),
        ValueType::Custom { name } => Cow::Owned(name.to_uppercase()),
    }
}

#[cfg(test)]
mod tests;
