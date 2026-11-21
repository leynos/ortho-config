//! Unit tests for roff escaping.
//!
//! Each encoder is pinned in three directions: ordinary text keeps its
//! meaning, metacharacters are neutralized, and adversarial payloads cannot
//! start a request line or leave a raw line break behind.

use super::*;
use rstest::rstest;

/// Reports whether any line would be read by roff as a request.
///
/// A line is a request when its first character is `.` or `'`. Encoded text
/// may still contain newlines, so every line is checked rather than only the
/// first.
pub(super) fn starts_a_request_line(rendered: &str) -> bool {
    rendered.lines().any(|line| line.starts_with(['.', '\'']))
}

/// Payloads that try to introduce a roff request line.
const INJECTION_PAYLOADS: &[&str] = &[
    "\n.so /etc/passwd",
    "\n.tm injected",
    "\r.tm injected",
    "\r\n.tm injected",
    "value\n.so /etc/passwd\n.tm injected",
    "'tm injected",
    ".so /etc/passwd",
    // groff discards these bytes while reading, so a leading one would hide
    // the `.` or `'` behind it and let the request run.
    "\0.tm injected",
    "\u{0b}.so /etc/passwd",
    "\u{1f}'tm injected",
];

#[rstest]
#[case("hello", "hello")]
#[case("path\\to\\file", "path\\\\to\\\\file")]
#[case("-flag", "\\-flag")]
#[case(".macro", "\\&.macro")]
#[case("'quote", "\\&'quote")]
#[case("normal-dash", "normal-dash")]
#[case("a.period", "a.period")]
fn escape_text_handles_special_chars(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(
        escape_text(input),
        expected,
        "text {input:?} should escape to {expected:?}"
    );
}

#[rstest]
fn escape_text_handles_multiline() {
    let input = "-first\n.second\n'third";
    let expected = "\\-first\n\\&.second\n\\&'third";
    assert_eq!(
        escape_text(input),
        expected,
        "each line should neutralize its own leading character"
    );
}

#[rstest]
fn escape_text_preserves_trailing_newline() {
    assert_eq!(
        escape_text("hello\n"),
        "hello\n",
        "a trailing LF should be preserved as one newline"
    );
    assert_eq!(
        escape_text("hello"),
        "hello",
        "text without a trailing break should gain none"
    );
}

#[rstest]
#[case("first\rsecond", "first\nsecond")]
#[case("first\r\nsecond", "first\nsecond")]
#[case("first\nsecond", "first\nsecond")]
#[case("first\r.tm injected", "first\n\\&.tm injected")]
#[case("first\r'tm injected", "first\n\\&'tm injected")]
#[case("a\n\nb", "a\n\nb")]
#[case("\0.so /etc/passwd", "\\&.so /etc/passwd")]
#[case("\u{0b}.tm injected", "\\&.tm injected")]
#[case("\u{1f}'tm injected", "\\&'tm injected")]
#[case("a\u{0}b", "ab")]
fn escape_text_treats_every_line_break_as_a_new_line(#[case] input: &str, #[case] expected: &str) {
    // groff 1.23 does not treat a lone CR as a line break: it warns about an
    // invalid input character and drops it. Passing it through would therefore
    // emit invalid input whose handling belongs to whichever processor reads
    // the page; treating it as a break keeps the encoded bytes valid for all
    // of them and stops a CR-separated request from reappearing as text.
    assert_eq!(
        escape_text(input),
        expected,
        "text {input:?} should break into the expected lines"
    );
}

#[rstest]
fn escape_text_preserves_a_trailing_carriage_return_as_a_newline() {
    assert_eq!(
        escape_text("hello\r"),
        "hello\n",
        "a trailing lone CR is a line break like any other"
    );
}

#[rstest]
#[case("hello", "hello")]
#[case("path\\to\\file", "path\\(rsto\\(rsfile")]
#[case("with \"quotes\"", "with \\(dqquotes\\(dq")]
#[case("mixed\\and\"both", "mixed\\(rsand\\(dqboth")]
#[case("2026-01-31\n.tm injected", "2026-01-31 .tm injected")]
#[case("line\r.so /etc/passwd", "line .so /etc/passwd")]
#[case(".leading", ".leading")]
#[case("a\0b", "ab")]
fn escape_quoted_arg_handles_special_chars(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(
        escape_quoted_arg(input),
        expected,
        "quoted argument {input:?} should escape to {expected:?}"
    );
}

#[rstest]
#[case("single line")]
#[case("a\nb")]
#[case("a\rb")]
#[case("a\r\nb")]
fn escape_quoted_arg_collapses_line_breaks(#[case] payload: &str) {
    let escaped = escape_quoted_arg(payload);
    assert!(
        !escaped.contains(['\n', '\r']),
        "a quoted argument must stay on one line: {escaped:?}"
    );
}

#[rstest]
#[case("plain", "plain")]
#[case("two words", "two\\ words")]
#[case("tab\there", "tab\\ here")]
#[case("line\nbreak", "line\\ break")]
#[case("line\rbreak", "line\\ break")]
#[case(".hidden", "\\&.hidden")]
#[case("'quote", "\\&'quote")]
#[case("back\\slash", "back\\(rsslash")]
#[case("say \"hi\"", "say\\ \\(dqhi\\(dq")]
#[case("\"quoted", "\\(dqquoted")]
#[case("\0.hidden", "\\&.hidden")]
#[case("\u{0b}'quote", "\\&'quote")]
#[case("a.b", "a.b")]
#[case("a'b", "a'b")]
fn escape_unquoted_arg_handles_special_chars(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(
        escape_unquoted_arg(input),
        expected,
        "unquoted argument {input:?} should escape to {expected:?}"
    );
}

#[rstest]
fn escape_quoted_arg_keeps_every_payload_on_one_line() {
    // A quoted slot ends at its closing quote, so the leading `.` of a payload
    // is harmless once no line break can move it to the start of a line.
    for payload in INJECTION_PAYLOADS {
        let escaped = escape_quoted_arg(payload);
        assert!(
            !escaped.contains(['\n', '\r']),
            "payload {payload:?} escaped to more than one line: {escaped:?}"
        );
        assert!(
            !escaped.chars().any(char::is_control),
            "payload {payload:?} kept a control character: {escaped:?}"
        );
    }
}

#[test]
fn escape_unquoted_arg_neutralizes_injection_payloads() {
    for payload in INJECTION_PAYLOADS {
        let escaped = escape_unquoted_arg(payload);
        assert!(
            !escaped.contains(['\n', '\r']),
            "payload {payload:?} escaped to more than one line: {escaped:?}"
        );
        assert!(
            !starts_a_request_line(&escaped),
            "payload {payload:?} escaped to a request line: {escaped:?}"
        );
        assert!(
            !escaped.chars().any(char::is_control),
            "payload {payload:?} kept a control character: {escaped:?}"
        );
    }
}

#[test]
fn escape_text_neutralizes_injection_payloads() {
    for payload in INJECTION_PAYLOADS {
        let escaped = escape_text(payload);
        assert!(
            !escaped.contains('\r'),
            "payload {payload:?} kept a raw CR: {escaped:?}"
        );
        assert!(
            !starts_a_request_line(&escaped),
            "payload {payload:?} escaped to a request line: {escaped:?}"
        );
        assert!(
            !escaped.chars().any(|ch| ch.is_control() && ch != '\n'),
            "payload {payload:?} kept a control character other than a line break: {escaped:?}"
        );
    }
}

#[rstest]
fn bold_wraps_and_escapes_text() {
    assert_eq!(bold("text"), "\\fBtext\\fR", "bold should wrap plain text");
    assert_eq!(
        bold("path\\to"),
        "\\fBpath\\\\to\\fR",
        "bold should escape backslashes"
    );
}

#[rstest]
fn italic_wraps_and_escapes_text() {
    assert_eq!(
        italic("text"),
        "\\fItext\\fR",
        "italic should wrap plain text"
    );
    assert_eq!(
        italic("path\\to"),
        "\\fIpath\\\\to\\fR",
        "italic should escape backslashes"
    );
}
