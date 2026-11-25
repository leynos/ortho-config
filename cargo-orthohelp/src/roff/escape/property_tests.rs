//! Property coverage for the roff encoders.
//!
//! The unit tables pin named payloads. These properties cover the open-ended
//! input space around them: whatever text package metadata carries, an
//! argument encoder must keep it on one line, and no encoder may place a
//! control character at the start of a line.

use proptest::prelude::*;

use super::tests::starts_a_request_line;
use super::{escape_quoted_arg, escape_text, escape_unquoted_arg};

/// A string guaranteed to contain at least one injection fragment.
fn hostile_string() -> impl Strategy<Value = String> {
    (
        any::<String>(),
        prop::sample::select(vec![
            "\n.tm injected",
            "\r.so /etc/passwd",
            "\r\n.tm injected",
            "'tm injected",
            ".so /etc/passwd",
            "\u{0}.tm injected",
            "\u{0b}.so /etc/passwd",
            "\\",
            "\"",
        ]),
        any::<String>(),
    )
        .prop_map(|(before, fragment, after)| format!("{before}{fragment}{after}"))
}

proptest! {
    #[test]
    fn quoted_arguments_never_span_lines(text in hostile_string()) {
        let escaped = escape_quoted_arg(&text);
        prop_assert!(
            !escaped.contains(['\n', '\r']),
            "quoted argument kept a line break: {escaped:?}"
        );
        // An unencoded quote could close the macro slot early.
        prop_assert!(
            !escaped.contains('"'),
            "quoted argument kept a raw quote: {escaped:?}"
        );
    }

    #[test]
    fn unquoted_arguments_are_single_safe_tokens(text in hostile_string()) {
        let escaped = escape_unquoted_arg(&text);
        prop_assert!(
            !escaped.contains(['\n', '\r']),
            "unquoted argument kept a line break: {escaped:?}"
        );
        prop_assert!(
            !escaped.contains('"'),
            "unquoted argument kept a raw quote: {escaped:?}"
        );
        prop_assert!(
            !starts_a_request_line(&escaped),
            "unquoted argument became a request: {escaped:?}"
        );
    }

    #[test]
    fn text_never_emits_a_control_line(text in hostile_string()) {
        let escaped = escape_text(&text);
        prop_assert!(
            !escaped.contains('\r'),
            "text kept a raw CR: {escaped:?}"
        );
        prop_assert!(
            !starts_a_request_line(&escaped),
            "text became a request: {escaped:?}"
        );
    }
}
