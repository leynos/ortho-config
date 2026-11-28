//! Parsing for container-level clap command attributes.
//!
//! These helpers read the `#[command(...)]`/`#[clap(...)]` attributes on a
//! struct or enum variant. Unreadable expressions are ignored rather than
//! rejected, because clap accepts values the derive cannot resolve statically.

use super::ClapRenameAll;
use super::clap_attrs::{consume_unknown_meta, is_clap_command_attribute};

/// Reads a clap key's argument from either `= expr` or `(expr)`.
///
/// clap accepts both spellings, and its method-call form accepts a trailing
/// comma. The parenthesized form always consumes its group, then reports the
/// content only when it parses as exactly one expression, so an unrelated
/// multi-argument method is ignored rather than turned into a derive error.
pub(super) fn clap_argument(
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<Option<syn::Expr>> {
    if meta.input.peek(syn::Token![=]) {
        return Ok(Some(meta.value()?.parse()?));
    }
    if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        let arguments =
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated(&content)?;
        if arguments.len() != 1 {
            return Ok(None);
        }
        return Ok(arguments.into_iter().next());
    }
    Ok(None)
}

/// Reads the string literal a clap key argument spells, when it is literal.
pub(super) fn literal_string(expression: Option<syn::Expr>) -> Option<syn::LitStr> {
    match expression {
        Some(syn::Expr::Lit(literal)) => match literal.lit {
            syn::Lit::Str(value) => Some(value),
            _ => None,
        },
        _ => None,
    }
}

/// Visits every literal argument of `key` in the container's command attrs.
///
/// Unrelated keys are consumed without interpretation, and a key whose value is
/// not a literal is ignored: clap accepts expressions, so the derive must not
/// reject an attribute it cannot resolve statically.
fn for_each_command_literal(
    attrs: &[syn::Attribute],
    key: &str,
    mut visit: impl FnMut(syn::LitStr),
) -> syn::Result<()> {
    for attr in attrs.iter().filter(|attr| is_clap_command_attribute(attr)) {
        let syn::Meta::List(_) = &attr.meta else {
            continue;
        };
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident(key) {
                return consume_unknown_meta(&meta);
            }
            if let Some(value) = literal_string(clap_argument(&meta)?) {
                visit(value);
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Reads the container's `#[command(rename_all = "...")]` casing rule.
///
/// An unknown casing is ignored so the derive keeps clap's default rather than
/// rejecting a declaration clap may interpret differently.
pub(crate) fn clap_rename_all(attrs: &[syn::Attribute]) -> syn::Result<ClapRenameAll> {
    let mut rename_all = ClapRenameAll::default();
    for_each_command_literal(attrs, "rename_all", |value| {
        if let Some(parsed) = ClapRenameAll::parse(&value.value()) {
            rename_all = parsed;
        }
    })?;
    Ok(rename_all)
}

/// Parse a clap command `name = "..."` override from an enum variant.
///
/// Both `name = "..."` and `name("...")` are accepted. Unlike the struct-level
/// command name, a variant name must resolve statically: the selector derives
/// its runtime label and identifiers from it, so a non-literal value is
/// rejected rather than silently replaced by a guess.
pub(crate) fn clap_variant_name(variant: &syn::Variant) -> syn::Result<Option<syn::LitStr>> {
    let mut name = None;
    for attr in variant
        .attrs
        .iter()
        .filter(|attr| is_clap_command_attribute(attr))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return consume_unknown_meta(&meta);
            }
            let expression = clap_argument(&meta)?.ok_or_else(|| {
                syn::Error::new_spanned(&meta.path, "clap command `name` requires a value")
            })?;
            let literal = literal_string(Some(expression)).ok_or_else(|| {
                syn::Error::new_spanned(&meta.path, "clap command `name` must be a string literal")
            })?;
            name = Some(literal);
            Ok(())
        })?;
    }
    Ok(name)
}
