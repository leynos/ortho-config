//! Rendered-token readers for the item-macro audit.
//!
//! `syn` renders a macro body or call as a token stream, and the audit reads
//! that rendering without parsing it again: whether the tokens can spell the
//! inventory's identifier, which macros they invoke, and how a path reads in a
//! diagnostic. Rendering separates tokens with spaces, which is what makes the
//! identifier immediately before a `!` the invoked macro's name.

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
        if let Some(name) = before
            .split_whitespace()
            .next_back()
            .filter(|word| is_identifier(word))
        {
            names.push(name.to_owned());
        }
        rest = after;
    }
    names
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
