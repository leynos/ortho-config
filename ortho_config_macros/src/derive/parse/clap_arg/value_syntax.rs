//! Literal and type inspection for clap argument declarations.
//!
//! Clap accepts arbitrary expressions for most keys. The documentation derive
//! can only describe values it can read statically, so these helpers return
//! `None` for anything else and let the field keep the generated surface.

use syn::meta::ParseNestedMeta;
use syn::{Expr, Lit, RangeLimits, Token, Type};

use super::super::{option_inner, vec_inner};

/// Reads a key's argument from either `= expr` or `(expr)`.
///
/// clap accepts both spellings for ordinary methods: `#[arg(num_args = 2)]`
/// and `#[arg(num_args(2))]`. Magic keys such as `long` use the first form,
/// but reading both keeps the parser aligned with clap's own grammar.
pub(super) fn argument(meta: &ParseNestedMeta) -> syn::Result<Option<Expr>> {
    if meta.input.peek(Token![=]) {
        return Ok(Some(meta.value()?.parse::<Expr>()?));
    }
    if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        return Ok(Some(content.parse::<Expr>()?));
    }
    Ok(None)
}

/// A parsed `num_args` range.
#[derive(Debug, Clone, Copy)]
pub(super) struct ValueRange {
    /// Minimum number of values accepted.
    pub(super) min: usize,
    /// Maximum number of values accepted.
    pub(super) max: usize,
}

/// Reports whether a clap key declares a default value.
pub(super) fn is_default_value_key(meta: &ParseNestedMeta) -> bool {
    meta.path.is_ident("default_value")
        || meta.path.is_ident("default_value_t")
        || meta.path.is_ident("default_value_os")
        || meta.path.is_ident("default_values")
        || meta.path.is_ident("default_values_t")
}

/// Parses a bare boolean or an explicit literal value.
///
/// Returns `None` when the argument is present but not a literal boolean, so
/// the caller can fall back to the generated surface rather than guessing.
pub(super) fn parse_bool(meta: &ParseNestedMeta) -> syn::Result<Option<bool>> {
    match argument(meta)? {
        None => Ok(Some(true)),
        Some(Expr::Lit(lit)) => Ok(literal_bool(&lit.lit)),
        Some(_) => Ok(None),
    }
}

/// Parses a string literal argument, reporting non-literals as `None`.
pub(super) fn parse_lit_str(meta: &ParseNestedMeta) -> syn::Result<Option<String>> {
    literal_argument(meta, |lit| match lit {
        Lit::Str(value) => Ok(Some(value.value())),
        _ => Ok(None),
    })
}

/// Parses an integer literal argument.
pub(super) fn parse_usize(meta: &ParseNestedMeta) -> syn::Result<Option<usize>> {
    literal_argument(meta, |lit| match literal_int(lit) {
        Some(value) => Ok(Some(value.base10_parse()?)),
        None => Ok(None),
    })
}

/// Applies `convert` to a literal argument, ignoring non-literal arguments.
fn literal_argument<T>(
    meta: &ParseNestedMeta,
    convert: impl FnOnce(&Lit) -> syn::Result<Option<T>>,
) -> syn::Result<Option<T>> {
    match argument(meta)? {
        Some(Expr::Lit(lit)) => convert(&lit.lit),
        _ => Ok(None),
    }
}

/// Extracts the boolean value of a literal, when it is one.
const fn literal_bool(lit: &Lit) -> Option<bool> {
    match lit {
        Lit::Bool(value) => Some(value.value),
        _ => None,
    }
}

/// Extracts the integer value of a literal, when it is one.
const fn literal_int(lit: &Lit) -> Option<&syn::LitInt> {
    match lit {
        Lit::Int(value) => Some(value),
        _ => None,
    }
}

/// Parses a literal `num_args` count or range.
pub(super) fn parse_value_range(meta: &ParseNestedMeta) -> syn::Result<Option<ValueRange>> {
    argument(meta)?.map_or(Ok(None), |expression| literal_value_range(&expression))
}

/// Converts a literal count or range expression into a value range.
fn literal_value_range(expression: &Expr) -> syn::Result<Option<ValueRange>> {
    match expression {
        Expr::Lit(literal) => literal_count(literal),
        Expr::Range(range) => Ok(literal_range(range)),
        _ => Ok(None),
    }
}

/// Converts a literal integer into an exact value count.
fn literal_count(literal: &syn::ExprLit) -> syn::Result<Option<ValueRange>> {
    let Some(value) = literal_int(&literal.lit) else {
        return Ok(None);
    };
    let count = value.base10_parse()?;
    Ok(Some(ValueRange {
        min: count,
        max: count,
    }))
}

/// Converts a literal range into its inclusive value bounds.
///
/// Returns `None` when a bound is present but not a literal integer, such as
/// `MIN..=1`. Treating an unreadable lower bound as zero would claim the value
/// is optional when it may be required, so the declaration is left unmodelled
/// instead of guessing.
fn literal_range(range: &syn::ExprRange) -> Option<ValueRange> {
    let start = match range.start.as_deref() {
        Some(expression) => Some(lit_usize(expression)?),
        None => None,
    };
    let end = match range.end.as_deref() {
        Some(expression) => Some(lit_usize(expression)?),
        None => None,
    };
    let min = start.unwrap_or(0);
    let max = match range.limits {
        RangeLimits::Closed(_) => end.unwrap_or(usize::MAX),
        RangeLimits::HalfOpen(_) => end.map_or(usize::MAX, |value| value.saturating_sub(1)),
    };
    Some(ValueRange { min, max })
}

/// Reads a literal integer expression when it is suitable as a range bound.
fn lit_usize(expr: &Expr) -> Option<usize> {
    let Expr::Lit(lit) = expr else {
        return None;
    };
    literal_int(&lit.lit).and_then(|value| value.base10_parse().ok())
}

/// Reads the final path segment of an `ArgAction` expression.
pub(super) fn action_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

/// Returns the one character a string literal spells, if it has exactly one.
pub(super) fn single_char(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let character = chars.next()?;
    chars.next().is_none().then_some(character)
}

/// Reports whether a type is exactly `bool`, the one type clap treats as a
/// value-less switch by default.
pub(super) fn is_bare_bool(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident("bool"))
}

/// Reports whether a type is a vector, including an optional vector.
pub(super) fn is_vec_field(ty: &Type) -> bool {
    let inner = option_inner(ty).unwrap_or(ty);
    vec_inner(inner).is_some()
}
