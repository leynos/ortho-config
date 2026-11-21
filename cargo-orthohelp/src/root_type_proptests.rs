//! Property tests for the root type grammar.
//!
//! The parser takes an open-ended input: a user string from `--root-type` or
//! `package.metadata.ortho_config.root_type`. The example-based cases in
//! `root_type_tests.rs` pin the grammar's named forms. These properties cover
//! the rest of that input space, where the security-relevant promise is that
//! anything the parser accepts renders as identifier path syntax and nothing
//! else, and that the rendering is stable if it is parsed again.
//!
//! Two properties run over arbitrary strings, which can only assert about the
//! inputs the parser accepts. Two more run over paths generated from the
//! grammar's own building blocks — raw identifiers, Unicode continuations, and
//! the whitespace variants `syn` tolerates — and require those to parse, so a
//! run cannot pass by accepting nothing.

use super::*;
use proptest::prelude::*;

/// Reports whether `rendered` is identifiers joined by `::`.
///
/// The bridge splices the rendering into generated Rust source, so this is the
/// shape the generator relies on: identifier characters only, with no
/// whitespace, punctuation, or operators that could close the path and start a
/// new expression. Identifier characters are Rust's own XID classes, because
/// an identifier may continue with a combining mark that `char::is_alphanumeric`
/// rejects, and a raw identifier keeps its `r#` prefix.
///
/// This is deliberately a property of the text rather than of rustc's exact
/// identifier rules. `syn` is more permissive than rustc about some code
/// points, and an identifier rustc refuses fails the bridge build with its own
/// diagnostic; the security promise here is only that nothing but a path can
/// reach the generated source.
fn is_identifier_path(rendered: &str) -> bool {
    rendered
        .split("::")
        .all(|segment| is_identifier(segment.strip_prefix("r#").unwrap_or(segment)))
}

/// Reports whether `ident` follows Rust's identifier grammar.
///
/// Rust starts an identifier with an XID start character or with `_`, but a
/// lone `_` is a pattern rather than an identifier, so an underscore start
/// needs at least one continuation character after it.
fn is_identifier(ident: &str) -> bool {
    let mut characters = ident.chars();
    match characters.next() {
        Some('_') => {
            characters
                .next()
                .is_some_and(unicode_ident::is_xid_continue)
                && characters.all(unicode_ident::is_xid_continue)
        }
        Some(first) => {
            unicode_ident::is_xid_start(first) && characters.all(unicode_ident::is_xid_continue)
        }
        None => false,
    }
}

/// Generates an identifier the grammar accepts in any segment position.
///
/// A bare `_` is excluded because it is a pattern, not an identifier, and the
/// reserved path starts are excluded because they are rejected wherever they
/// appear first. Generating either here would test the rejection path rather
/// than the rendering.
fn generated_ident() -> impl Strategy<Value = String> {
    "[A-Za-z_][A-Za-z0-9_]{0,8}".prop_filter(
        "an identifier must not be a reserved path start or a bare `_`",
        |ident| !matches!(ident.as_str(), "_" | "crate" | "self" | "super" | "Self"),
    )
}

/// Generates a crate name for the qualification the renderer applies.
///
/// The name starts with a letter so it is always an identifier, and it avoids
/// keywords because it is spliced into the rendered text as a path segment.
fn generated_crate_ident() -> impl Strategy<Value = String> {
    "[A-Za-z][A-Za-z0-9_]{0,8}".prop_filter("a crate name must not be a keyword", |ident| {
        !matches!(ident.as_str(), "crate" | "self" | "super" | "Self")
    })
}

/// Generates a path segment, weighted so the interesting shapes appear often.
fn path_segment() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => generated_ident(),
        1 => Just("r#type".to_owned()),
        1 => Just("q\u{301}".to_owned()),
        1 => Just("ς".to_owned()),
    ]
}

/// Generates a path the grammar must accept, in a variety of spellings.
///
/// The variants are the ones `syn` tolerates and the renderer promises to
/// canonicalize: spacing around `::`, a leading space, a leading `crate`
/// segment, and segments that are raw identifiers or carry combining marks.
fn supported_root_type() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(path_segment(), 1..=3),
        prop_oneof![Just("::"), Just(" :: "), Just(":: ")],
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(segments, joiner, should_start_with_crate, should_lead_with_space)| {
                let mut path = segments.join(joiner);
                if should_start_with_crate {
                    path = format!("crate{joiner}{path}");
                }
                if should_lead_with_space {
                    path = format!(" {path}");
                }
                path
            },
        )
}

proptest! {
    /// Any accepted input renders as identifier path syntax only.
    #[test]
    fn accepted_root_type_renders_only_path_syntax(
        raw in ".*",
        crate_ident in generated_crate_ident(),
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

    /// Any accepted rendering is already qualified, so parsing it again cannot
    /// change it.
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

    /// A path built from the grammar's own shapes parses and renders as path
    /// syntax, so the arbitrary-string properties cannot pass vacuously.
    #[test]
    fn supported_root_type_renders_only_path_syntax(
        path in supported_root_type(),
        crate_ident in generated_crate_ident(),
    ) {
        let rendered = parse_root_type(&path, &crate_ident).map_err(|error| {
            TestCaseError::fail(format!("{path:?} should parse, but: {error}"))
        })?;
        prop_assert!(
            is_identifier_path(&rendered),
            "{:?} rendered as {:?}, which is not identifier path syntax",
            path,
            rendered,
        );
    }

    /// Every accepted spelling of a path renders to the same qualified form,
    /// whatever crate it is parsed in.
    #[test]
    fn supported_root_type_rendering_is_stable(path in supported_root_type()) {
        let rendered = parse_root_type(&path, "demo").map_err(|error| {
            TestCaseError::fail(format!("{path:?} should parse, but: {error}"))
        })?;
        let reparsed = parse_root_type(&rendered, "other").map_err(|error| {
            TestCaseError::fail(format!("canonical rendering {rendered:?} should parse, but: {error}"))
        })?;
        prop_assert_eq!(reparsed, rendered);
    }
}
