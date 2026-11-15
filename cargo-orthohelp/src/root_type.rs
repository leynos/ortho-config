//! Validation and canonicalization of the configured root type path.
//!
//! The bridge generator splices the root type into generated Rust source, so
//! the value must be a plain type path before it can be used. This module owns
//! that grammar: it parses the caller's value, rejects forms that cannot name a
//! configuration type from the separately compiled bridge, and renders the
//! accepted path from its syntax tree rather than from the original text.

use crate::error::OrthohelpError;

/// Parses and canonicalizes the root configuration type path.
///
/// The root type is spliced into generated bridge source, so this accepts only
/// a plain path of identifiers. Parsing happens before qualification: a leading
/// `crate` segment is rewritten to the package crate name because the bridge is
/// a separate crate, a bare type name receives the package crate prefix, and an
/// already-qualified path keeps its own leading segment. The returned value is
/// rendered from the parsed path, never from the original text, so a statement
/// or expression cannot reach the generated source.
///
/// # Errors
///
/// Returns [`OrthohelpError::InvalidRootType`] when the value does not parse as
/// a type path, or when it carries a qualified self type, a leading `::`,
/// generic or parenthesized path arguments, or a leading segment that cannot
/// name a type in the bridge crate.
pub(crate) fn parse_root_type(raw: &str, crate_ident: &str) -> Result<String, OrthohelpError> {
    let parsed = syn::parse_str::<syn::TypePath>(raw)
        .map_err(|error| invalid_root_type(raw, error.to_string()))?;
    ensure_supported_root_type(&parsed, raw)?;
    Ok(render_root_type(&parsed, crate_ident))
}

/// Rejects parsed type paths that fall outside the supported root type grammar.
fn ensure_supported_root_type(parsed: &syn::TypePath, raw: &str) -> Result<(), OrthohelpError> {
    if parsed.qself.is_some() {
        return Err(invalid_root_type(
            raw,
            "qualified self types such as `<Type as Trait>::Assoc` are not supported",
        ));
    }

    if parsed.path.leading_colon.is_some() {
        return Err(invalid_root_type(
            raw,
            "global paths starting with `::` are not supported",
        ));
    }

    if let Some(segment) = parsed
        .path
        .segments
        .iter()
        .find(|segment| !matches!(segment.arguments, syn::PathArguments::None))
    {
        return Err(invalid_root_type(
            raw,
            format!(
                "generic or parenthesized arguments on `{}` are not supported",
                segment.ident
            ),
        ));
    }

    ensure_type_path_start(parsed, raw)
}

/// Rejects leading segments that name a module or the bridge's own crate.
///
/// `crate::Config` is accepted here and rewritten by [`render_root_type`], but
/// a bare `crate` and any `self`, `super`, or `Self` start cannot resolve to a
/// configuration type from the separately compiled bridge.
fn ensure_type_path_start(parsed: &syn::TypePath, raw: &str) -> Result<(), OrthohelpError> {
    let Some(first) = parsed.path.segments.first() else {
        return Err(invalid_root_type(raw, "root type path is empty"));
    };
    let ident = first.ident.to_string();

    if matches!(ident.as_str(), "self" | "super" | "Self") {
        return Err(invalid_root_type(
            raw,
            format!("`{ident}` cannot start a root type path"),
        ));
    }

    if ident == "crate" && parsed.path.segments.len() == 1 {
        return Err(invalid_root_type(
            raw,
            "a bare `crate` does not name a type",
        ));
    }

    Ok(())
}

/// Renders the accepted path from its parsed segments rather than the input text.
///
/// Qualification is decided from the parsed segments, so spacing variants such
/// as `crate :: Config` and a leading space qualify exactly like their compact
/// spellings.
fn render_root_type(parsed: &syn::TypePath, crate_ident: &str) -> String {
    let mut segments = parsed
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();

    match segments.as_mut_slice() {
        [first, ..] if first.as_str() == "crate" => crate_ident.clone_into(first),
        [only] => *only = format!("{crate_ident}::{only}"),
        _ => {}
    }

    segments.join("::")
}

/// Builds a diagnostic that preserves the caller's original spelling.
fn invalid_root_type(value: &str, message: impl Into<String>) -> OrthohelpError {
    OrthohelpError::InvalidRootType {
        value: value.to_owned(),
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "root_type_tests.rs"]
mod tests;
