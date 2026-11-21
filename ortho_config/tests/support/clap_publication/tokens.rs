//! Rendered-token readers for the item-macro audit.
//!
//! `syn` renders a macro body or call as a token stream, and the audit reads
//! that rendering without parsing it again: whether the tokens can spell the
//! inventory's identifier, which macros they invoke, and how a path reads in a
//! diagnostic. Rendering separates tokens with spaces, which is what makes the
//! identifier immediately before a `!` the invoked macro's name.
//!
//! A name can carry `r#`: `proc-macro2` prints a raw identifier with its
//! prefix, and Rust resolves `r#include` to the same macro as `include`,
//! `r#ErrorKind` to the same type as `ErrorKind`, and `#[r#cfg]` to the same
//! attribute as `#[cfg]`. The prefix is removed before a name is resolved or
//! compared, so a declaration cannot hide behind a spelling the audit declines
//! to recognize. [`identifier_name`] is the one place that rule lives, because a
//! spelling normalized in one module and compared raw in another is exactly the
//! gap the fixtures beside it exist to catch.

use syn::Path;

/// The identifier a generated inventory would have to spell.
const INVENTORY: &str = "ErrorKind";

/// Whether the tokens can spell the inventory's identifier.
pub(super) fn mentions_inventory(tokens: &str) -> bool {
    tokens
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .any(|token| token == INVENTORY)
}

/// The macros a rendered transcription invokes.
///
/// A name the audit cannot resolve is returned as well and fails it by
/// resolving to no transcription.
pub(super) fn invoked_macro_names(tokens: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = tokens;
    while let Some((before, after)) = rest.split_once('!') {
        if let Some(name) = before.split_whitespace().next_back().and_then(macro_name) {
            names.push(name.to_owned());
        }
        rest = after;
    }
    names
}

/// The name an identifier denotes, with a raw identifier normalized.
///
/// Rust treats `r#compile_error` and `compile_error` as one identifier, so a
/// definition, a call, a `use` binding, a type name, and a module name all have
/// to be recorded the same way. Comparing rendered spellings instead lets a
/// definition under the raw prefix resolve no call, which is how a splice hides
/// behind a name the audit trusts.
pub(super) fn identifier_name(ident: &syn::Ident) -> String {
    strip_raw_prefix(&ident.to_string()).to_owned()
}

/// The same normalization for a name already rendered as text.
fn strip_raw_prefix(rendered: &str) -> &str {
    rendered.strip_prefix("r#").unwrap_or(rendered)
}

/// The macro name a rendered word spells, with a raw identifier normalized.
///
/// Rust treats `r#include` and `include` as one identifier, so dropping the
/// prefix is what makes the name resolve the way the compiler resolves it.
/// Leaving the raw spelling unrecognized would instead treat the invocation as
/// naming no macro at all, which is how a wrapper can hide its splice.
fn macro_name(word: &str) -> Option<&str> {
    let plain = strip_raw_prefix(word);
    is_identifier(plain).then_some(plain)
}

/// Whether a rendered token is a bare identifier.
fn is_identifier(word: &str) -> bool {
    let Some(first) = word.chars().next() else {
        return false;
    };
    let starts_ident = first.is_alphabetic() || first == '_';
    starts_ident
        && word
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
}

/// Render a macro path the way the source spells it.
pub(super) fn quote_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
