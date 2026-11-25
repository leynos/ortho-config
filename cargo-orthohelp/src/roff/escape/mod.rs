//! Roff text escaping and formatting utilities.
//!
//! Roff reads a man page one physical line at a time: a line whose first
//! character is `.` or `'` is a request, and every `\` introduces an escape
//! sequence. A newline that reaches the output from untrusted metadata
//! therefore starts a request line of the author's choosing, so each value
//! must be encoded for the syntactic context it lands in.
//!
//! Three encoders cover those contexts, and every renderer sink must use the
//! one that matches it:
//!
//! - [`escape_quoted_arg`] for a macro argument that may legitimately contain
//!   spaces, whether the caller wraps it in double quotes (`.TH "NAME"`) or
//!   passes it as the remainder of the request line (`.SH NAME`).
//! - [`escape_unquoted_arg`] for a single bare token after a macro, such as
//!   the `.UR` target or the `.BR` command name, where whitespace would end
//!   the argument.
//! - [`escape_text`] for body lines, where each line is emitted verbatim apart
//!   from being unable to begin with a control character.
//!
//! All three encode CR and LF, so no value can introduce a request line; the
//! argument encoders keep a value on one line, while [`escape_text`] starts a
//! new output line for each break and protects the beginning of every line.
//!
//! [`bold`] and [`italic`] are font wrappers over [`escape_text`], and the
//! flag and placeholder formatters live in the private `flags` submodule,
//! re-exported here so call sites keep one import path.

mod flags;

pub use flags::{
    format_flag, format_flag_with_optional_value, format_flag_with_value, format_option,
    value_type_placeholder,
};

/// Escapes body text so no line can begin with a roff control character.
///
/// Body text is emitted one physical line at a time, and roff reads a line
/// whose first character is `.` or `'` as a request. Every line break
/// therefore starts a new output line whose first character is neutralized,
/// and a lone CR counts as a line break in the same way as LF and CRLF does.
///
/// Handles, on every line:
/// - Backslashes: `\` -> `\\`
/// - Leading `-`: `-` -> `\-` (prevents option interpretation)
/// - Leading `.`: `.` -> `\&.` (prevents request interpretation)
/// - Leading `'`: `'` -> `\&'` (prevents request interpretation)
///
/// A trailing line break is preserved as exactly one newline.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::escape_text;
///
/// assert_eq!(escape_text("hello"), "hello", "plain text should be unchanged");
/// assert_eq!(
///     escape_text("path\\to\\file"),
///     "path\\\\to\\\\file",
///     "backslashes should be doubled"
/// );
/// assert_eq!(escape_text("-flag"), "\\-flag", "a leading dash should be escaped");
/// assert_eq!(
///     escape_text("first\r.so /etc/passwd"),
///     "first\n\\&.so /etc/passwd",
///     "a CR should start a new neutralized line"
/// );
/// ```
#[must_use]
pub fn escape_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + text.len().div_ceil(8));
    let mut rest = text;
    let mut is_first_line = true;

    while let Some((line, remainder)) = split_at_line_break(rest) {
        if !is_first_line {
            result.push('\n');
        }
        escape_line(line, &mut result);
        is_first_line = false;
        rest = remainder;

        if rest.is_empty() {
            // The text ended with a line break, and roff output keeps exactly
            // one; any further breaks were consumed as blank lines above.
            result.push('\n');
            return result;
        }
    }

    if !is_first_line {
        result.push('\n');
    }
    escape_line(rest, &mut result);
    result
}

/// Splits text after its first LF, CRLF or lone CR.
///
/// Returns the line without its terminator and the text that follows it, or
/// `None` when the text holds no further line break.
fn split_at_line_break(text: &str) -> Option<(&str, &str)> {
    let index = text.find(['\n', '\r'])?;
    // `find` reports a character boundary, so `split_at` cannot panic here.
    let (line, tail) = text.split_at(index);
    let break_len = usize::from(tail.starts_with("\r\n")) + 1;
    let (_, remainder) = tail.split_at(break_len);
    Some((line, remainder))
}

/// Returns whether `ch` is a control character the encoders drop.
///
/// Groff deletes an invalid input character (NUL, vertical tab, most C0
/// controls, DEL, and C1 controls) while reading, *before* it decides whether
/// the line starts a request. Emitting such a byte at the start of a line
/// therefore hides the real first character: for `\0.tm evil`, groff drops
/// the NUL and then executes `.tm`. Dropping the same characters ourselves
/// keeps the leading-character check pointed at the character groff will
/// actually see. A tab is legitimate in verbatim text, and LF and CR are line
/// breaks the encoders handle before this predicate applies.
fn is_dropped_control(ch: char) -> bool {
    ch.is_control() && !matches!(ch, '\t' | '\n' | '\r')
}

/// Escapes one line, treating its first surviving character specially to
/// prevent roff macro execution.
fn escape_line(line: &str, result: &mut String) {
    let mut chars = line.chars().filter(|ch| !is_dropped_control(*ch));

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

/// Writes a roff-safe form for a special first character and reports whether it handled it.
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

/// Escapes backslashes wherever they occur so input cannot introduce roff escapes.
fn push_escaped_char(ch: char, result: &mut String) {
    match ch {
        '\\' => result.push_str("\\\\"),
        _ => result.push(ch),
    }
}

/// Escapes one macro argument that may contain spaces.
///
/// Use this for a value that occupies a whole macro slot: inside double quotes
/// such as `.TH "NAME" "1" "DATE"`, or unquoted as the remainder of the
/// request line such as `.SH NAME`. Spaces survive because roff keeps a
/// quoted slot together and reads an unquoted heading as the rest of the line,
/// so a multi-word heading still renders as one heading.
///
/// Handles:
/// - CR and LF: each becomes a single space, so the value stays on one
///   physical line and cannot start a new request
/// - Backslashes: `\` -> `\(rs`, a glyph that survives the extra layer of
///   interpretation a macro argument receives
/// - Double quotes: `"` -> `\(dq`
/// - Other control characters: dropped, because groff discards them while
///   reading and they would otherwise hide the character that follows
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::escape_quoted_arg;
///
/// assert_eq!(escape_quoted_arg("My App"), "My App", "spaces should survive");
/// assert_eq!(
///     escape_quoted_arg("say \"hi\""),
///     "say \\(dqhi\\(dq",
///     "quotes should be encoded"
/// );
/// assert_eq!(
///     escape_quoted_arg("2026-01-31\n.tm injected"),
///     "2026-01-31 .tm injected",
///     "a line break should collapse to a space"
/// );
/// assert_eq!(
///     escape_quoted_arg("My\0App"),
///     "MyApp",
///     "a discarded control character should be dropped"
/// );
/// ```
#[must_use]
pub fn escape_quoted_arg(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + text.len().div_ceil(8));
    for ch in text.chars() {
        match ch {
            // A macro argument is interpreted once more than body text, which
            // would turn `\\\\` back into an escape. The glyph survives
            // that extra layer as a literal backslash.
            '\\' => result.push_str("\\(rs"),
            '"' => result.push_str("\\(dq"),
            '\n' | '\r' => result.push(' '),
            dropped if is_dropped_control(dropped) => {}
            _ => result.push(ch),
        }
    }
    result
}

/// Escapes a value that must stay a single unquoted macro argument.
///
/// Use this for a bare token after a macro, such as the `.UR` target or the
/// `.BR` command name, where roff ends the argument at the first space.
///
/// Handles:
/// - CR, LF, spaces and tabs: each becomes `\ `, an unpaddable space, so the
///   value stays one argument and cannot start a new request
/// - Backslashes: `\` -> `\(rs`, a glyph that survives the extra layer of
///   interpretation a macro argument receives
/// - Double quotes: `"` -> `\(dq`, so a quote can never open a quoted string
/// - A leading `.` or `'`: prefixed with `\&`, a zero-width character, so even
///   a value moved to the start of a line cannot become a request
/// - Other control characters: dropped before the leading-character check, so
///   one cannot hide a `.` or `'` that groff would then act on
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::escape_unquoted_arg;
///
/// assert_eq!(
///     escape_unquoted_arg("my-app"),
///     "my-app",
///     "a plain token should be unchanged"
/// );
/// assert_eq!(
///     escape_unquoted_arg("two words"),
///     "two\\ words",
///     "a space should be encoded"
/// );
/// assert_eq!(
///     escape_unquoted_arg(".hidden"),
///     "\\&.hidden",
///     "a leading dot should be neutralized"
/// );
/// assert_eq!(
///     escape_unquoted_arg("\0.so /etc/passwd"),
///     "\\&.so\\ /etc/passwd",
///     "a NUL should not hide the leading dot"
/// );
/// assert_eq!(
///     escape_unquoted_arg("a\\b"),
///     "a\\(rsb",
///     "a backslash should become a glyph"
/// );
/// ```
#[must_use]
pub fn escape_unquoted_arg(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + text.len().div_ceil(8));
    // The leading check follows what has been *emitted*, not the source
    // offset: a dropped control character must not shift the dot or quote out
    // of the leading position.
    let mut is_first = true;

    for ch in text.chars() {
        if is_dropped_control(ch) {
            continue;
        }
        match ch {
            '\n' | '\r' | ' ' | '\t' => result.push_str("\\ "),
            // See `escape_quoted_arg`: a bare doubled backslash would be
            // interpreted as an escape inside a macro argument.
            '\\' => result.push_str("\\(rs"),
            '"' => result.push_str("\\(dq"),
            '.' | '\'' if is_first => {
                result.push_str("\\&");
                result.push(ch);
            }
            _ => result.push(ch),
        }
        is_first = false;
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

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod tests;
