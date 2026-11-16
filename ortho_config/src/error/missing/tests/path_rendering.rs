//! Tests for rendering configuration paths as quoted TOML keys.
//!
//! A rendered path reaches a terminal, a log line, and a suggested file
//! section, so every segment outside TOML's bare-key alphabet must be quoted
//! and every control character escaped. These cases live beside the payload
//! tests rather than inside them so neither file outgrows the module line
//! budget.

use super::*;

#[test]
fn dotted_segments_are_quoted_to_keep_paths_unambiguous() {
    let path = ConfigPath::new(["a.b", "c"]);
    assert_eq!(
        path.render(),
        "\"a.b\".c",
        "a literal dot in a segment must be quoted so it cannot read as nesting"
    );
    assert_eq!(
        ConfigPath::new(["a", "b"]).render(),
        "a.b",
        "two plain segments must join through the ordinary separator"
    );
    assert_eq!(
        ConfigPath::new(["with space"]).render(),
        "\"with space\"",
        "a space is outside the bare-key alphabet, so the key needs quotes"
    );
    assert_eq!(
        ConfigPath::new(Vec::<String>::new()).render(),
        "",
        "an empty path has no segment to render"
    );
}

#[test]
fn keys_outside_the_bare_alphabet_are_quoted_and_escaped() {
    assert_eq!(
        ConfigPath::new(["Host=Name"]).render(),
        "\"Host=Name\"",
        "`=` is outside TOML's bare-key alphabet, so the key needs quotes"
    );
    assert_eq!(
        ConfigPath::new(["say \"hi\""]).render(),
        "\"say \\\"hi\\\"\"",
        "a quote inside a quoted key must be escaped"
    );
    assert_eq!(
        ConfigPath::new(["a_b-c9"]).render(),
        "a_b-c9",
        "the bare alphabet must stay unquoted"
    );
}

/// Control characters inside a quoted key are escaped, never emitted raw.
///
/// The rendered report and the suggested file section spell keys through one
/// routine, so a key that reaches a terminal, a log, or a TOML document must
/// not carry a raw newline or NUL through either. The named escapes are the
/// ones TOML defines; every other control character, including DEL, uses the
/// `\u` form.
#[test]
fn control_characters_in_a_quoted_key_are_escaped() {
    assert_eq!(
        ConfigPath::new(["line\nbreak"]).render(),
        "\"line\\nbreak\"",
        "a newline must be escaped rather than breaking the rendered line"
    );
    assert_eq!(
        ConfigPath::new(["tab\there"]).render(),
        "\"tab\\there\"",
        "a tab must use TOML's short escape"
    );
    assert_eq!(
        ConfigPath::new(["carriage\rreturn"]).render(),
        "\"carriage\\rreturn\"",
        "a carriage return must be escaped rather than returning the cursor"
    );
    assert_eq!(
        ConfigPath::new(["nul\0byte"]).render(),
        "\"nul\\u0000byte\"",
        "a NUL must be escaped rather than truncating the rendered text"
    );
    assert_eq!(
        ConfigPath::new(["del\u{7f}"]).render(),
        "\"del\\u007F\"",
        "DEL is a control character, so a quoted key escapes it too"
    );
}
