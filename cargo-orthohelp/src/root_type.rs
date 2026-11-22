//! Validation and canonicalization of the configured root type path.
//!
//! The bridge generator splices the root type into generated Rust source, so
//! the value must be a plain type path before it can be used. This module owns
//! that grammar: it parses the caller's value, rejects forms that cannot name a
//! configuration type from the separately compiled bridge, and renders the
//! accepted path from its syntax tree rather than from the original text.

use crate::error::OrthohelpError;

/// The root type exactly as the caller spelled it.
///
/// Diagnostics quote this value, so it is kept apart from the parsed path and
/// from the rendering: only the parser ever reads it.
#[derive(Clone, Copy)]
pub(crate) struct RawRootType<'a>(&'a str);

impl<'a> RawRootType<'a> {
    /// Wraps the caller's spelling.
    pub(crate) const fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Returns the caller's spelling.
    const fn as_str(self) -> &'a str {
        self.0
    }
}

/// The package crate name that a root type is qualified with.
///
/// The bridge is a separate crate, so a `crate`-prefixed or bare root type
/// has to be rewritten to this name.
#[derive(Clone, Copy)]
pub(crate) struct CrateIdent<'a>(&'a str);

impl<'a> CrateIdent<'a> {
    /// Wraps the package crate name.
    pub(crate) const fn new(value: &'a str) -> Self {
        Self(value)
    }
}

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
/// generic or parenthesized path arguments, or a `crate`, `self`, `super`, or
/// `Self` segment that cannot name a type in the bridge crate.
pub(crate) fn parse_root_type(
    raw: RawRootType<'_>,
    crate_ident: CrateIdent<'_>,
) -> Result<String, OrthohelpError> {
    let parsed = syn::parse_str::<syn::TypePath>(raw.as_str())
        .map_err(|error| invalid_root_type(raw, error.to_string()))?;
    ensure_supported_root_type(&parsed, raw)?;
    Ok(render_root_type(&parsed, crate_ident))
}

/// Rejects parsed type paths that fall outside the supported root type grammar.
fn ensure_supported_root_type(
    parsed: &syn::TypePath,
    raw: RawRootType<'_>,
) -> Result<(), OrthohelpError> {
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

    ensure_no_relative_keyword_after_first(parsed, raw)?;
    ensure_type_path_start(parsed, raw)
}

/// Rejects `crate`, `self`, `super`, and `Self` outside the first segment.
///
/// Rust allows these keywords only at the start of a path. `syn` parses them
/// in any position, so `demo::crate::Config` would otherwise reach the bridge
/// and fail there with a rustc diagnostic instead of this one.
fn ensure_no_relative_keyword_after_first(
    parsed: &syn::TypePath,
    raw: RawRootType<'_>,
) -> Result<(), OrthohelpError> {
    let Some(segment) = parsed
        .path
        .segments
        .iter()
        .skip(1)
        .find(|segment| is_relative_keyword(&segment.ident))
    else {
        return Ok(());
    };

    Err(invalid_root_type(
        raw,
        format!(
            "`{}` cannot appear after the first segment of a root type path",
            segment.ident
        ),
    ))
}

/// Reports whether `ident` is one of Rust's path-relative keywords.
fn is_relative_keyword(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "crate" | "self" | "super" | "Self"
    )
}

/// Rejects leading segments that name a module or the bridge's own crate.
///
/// `crate::Config` is accepted here and rewritten by [`render_root_type`], but
/// a bare `crate` and any `self`, `super`, or `Self` start cannot resolve to a
/// configuration type from the separately compiled bridge.
fn ensure_type_path_start(
    parsed: &syn::TypePath,
    raw: RawRootType<'_>,
) -> Result<(), OrthohelpError> {
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
fn render_root_type(parsed: &syn::TypePath, crate_ident: CrateIdent<'_>) -> String {
    let mut segments = parsed
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();

    match segments.as_mut_slice() {
        [first, ..] if first.as_str() == "crate" => crate_ident.0.clone_into(first),
        [only] => *only = format!("{}::{only}", crate_ident.0),
        _ => {}
    }

    segments.join("::")
}

/// Builds a diagnostic that preserves the caller's original spelling.
fn invalid_root_type(value: RawRootType<'_>, message: impl Into<String>) -> OrthohelpError {
    OrthohelpError::InvalidRootType {
        value: value.as_str().to_owned(),
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "root_type_proptests.rs"]
mod proptests;

#[cfg(test)]
#[path = "root_type_tests.rs"]
mod tests;
