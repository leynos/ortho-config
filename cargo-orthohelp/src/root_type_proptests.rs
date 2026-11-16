//! Property tests for the root type grammar.
//!
//! The parser takes an open-ended input: a user string from `--root-type` or
//! `package.metadata.ortho_config.root_type`. The example-based cases in
//! `root_type_tests.rs` pin the grammar's named forms. These properties cover
//! the rest of that input space, where the security-relevant promise is that
//! anything the parser accepts renders as identifier path syntax and nothing
//! else, and that the rendering is stable if it is parsed again.

use super::*;
use proptest::prelude::*;

/// Reports whether `rendered` is identifiers joined by `::`.
///
/// The bridge splices the rendering into generated Rust source, so this is the
/// shape the generator relies on: no whitespace, punctuation, or operators.
fn is_identifier_path(rendered: &str) -> bool {
    rendered.split("::").all(|segment| {
        let mut characters = segment.chars();
        matches!(
            characters.next(),
            Some(first) if first.is_ascii_alphabetic() || first == '_'
        ) && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    })
}

proptest! {
    /// Accepted input renders as identifier path syntax only.
    #[test]
    fn accepted_root_type_renders_only_path_syntax(
        raw in ".*",
        crate_ident in "[A-Za-z_][A-Za-z0-9_]*",
    ) {
        if let Ok(rendered) = parse_root_type(&raw, &crate_ident) {
            prop_assert!(
                is_identifier_path(&rendered),
                "{:?} rendered as {:?}, which is not identifier path syntax",
                raw,
                rendered,
            );
        }
    }

    /// A rendering is already qualified, so parsing it again cannot change it.
    ///
    /// The second parse uses a different crate name on purpose: a rendering
    /// that still depended on the crate it was produced in would be rewritten.
    #[test]
    fn accepted_root_type_rendering_is_stable(raw in ".*") {
        if let Ok(rendered) = parse_root_type(&raw, "demo") {
            let reparsed = parse_root_type(&rendered, "other")
                .expect("a canonical rendering should parse");
            prop_assert_eq!(reparsed, rendered);
        }
    }
}
