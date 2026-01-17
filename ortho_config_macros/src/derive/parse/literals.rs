//! Literal parsing helpers for derive attributes.

use syn::{Lit, LitStr};

/// Parses a literal from a field attribute using `extractor`.
///
/// # Examples
///
/// ```ignore
/// # use syn::meta::ParseNestedMeta;
/// # use syn::{Lit, LitStr};
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let s: LitStr = parse_lit(meta, "cli_long", |lit| match lit {
///     Lit::Str(s) => Some(s),
///     _ => None,
/// })?;
/// # Ok(())
/// # }
/// ```
fn parse_lit<T, F>(
    meta: &syn::meta::ParseNestedMeta,
    key: &str,
    extractor: F,
) -> Result<T, syn::Error>
where
    F: FnOnce(Lit) -> Option<T>,
{
    let literal = meta.value()?.parse::<Lit>()?;
    let span = literal.span();
    extractor(literal).ok_or_else(|| {
        let type_name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("literal")
            .to_lowercase();
        let display_type = match type_name.as_str() {
            "litstr" => "string",
            other => other,
        };
        syn::Error::new(span, format!("{key} must be a {display_type}"))
    })
}

/// Parses a string literal from a field attribute.
///
/// # Examples
///
/// ```rust,ignore
/// // Build a synthetic attribute and visit its nested meta so we can call into
/// // the parsing helper in this crate. The nightly-2025-09-16 toolchain that
/// // backs this repository currently ICEs when compiling the snippet, so the
/// // example is marked `ignore` until the regression is fixed.
/// use syn::Attribute;
/// let attr: Attribute = syn::parse_quote!(#[ortho_config(cli_long = "name")]);
/// attr.parse_nested_meta(|meta| {
///     let s = ortho_config_macros::__doc_lit_str(&meta, "cli_long")?;
///     assert_eq!(s.value(), "name");
///     Ok(())
/// }).unwrap();
/// ```
pub(crate) fn lit_str(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<LitStr, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Str(s) => Some(s),
        _ => None,
    })
}

/// Parses a character literal from a field attribute.
///
/// # Examples
///
/// ```rust,ignore
/// # use syn::meta::ParseNestedMeta;
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let c = lit_char(meta, "cli_short")?;
/// assert_eq!(c, 'n');
/// # Ok(())
/// # }
/// ```
pub(crate) fn lit_char(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<char, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Char(c) => Some(c.value()),
        _ => None,
    })
}

pub(crate) fn lit_bool(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<bool, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Bool(b) => Some(b.value),
        _ => None,
    })
}

// Expose a thin wrapper for doctests without leaking internals into the public
// API in normal builds. This allows examples to type-check while keeping
// `lit_str` private outside of tests/doctests.
#[cfg(any(test, doctest))]
#[doc(hidden)]
pub fn __doc_lit_str(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<LitStr, syn::Error> {
    lit_str(meta, key)
}
