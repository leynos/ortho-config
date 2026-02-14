//! Parsing helpers for clap field attributes.
//!
//! These helpers extract metadata from `#[arg(...)]` and `#[clap(...)]`
//! attributes without taking a dependency on clap itself.

use syn::Expr;

/// Returns `true` when the attribute is `#[arg(...)]` or `#[clap(...)]`.
pub(crate) fn is_clap_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("arg") || attr.path().is_ident("clap")
}

/// Parse a clap argument `id = "..."` override from a nested meta item.
///
/// When the meta item is not `id`, this function is a no-op (but will still
/// consume any `= <value>` tokens to keep parsing in sync). When an `id` value
/// is supplied it is stored in `existing_id`, and a duplicate `id` triggers a
/// `syn::Error` with the same message used elsewhere in the derive.
pub(crate) fn parse_id_from_meta(
    meta: &syn::meta::ParseNestedMeta<'_>,
    existing_id: &mut Option<syn::LitStr>,
) -> syn::Result<()> {
    if !meta.path.is_ident("id") {
        if meta.input.peek(syn::Token![=]) {
            let value = meta.value()?;
            let _: syn::Expr = value.parse()?;
        }
        return Ok(());
    }

    if existing_id.is_some() {
        return Err(syn::Error::new_spanned(
            &meta.path,
            "duplicate clap argument `id` override",
        ));
    }

    let Ok(value) = meta.value() else {
        return Ok(());
    };
    let lit: syn::LitStr = value.parse().map_err(|_| {
        syn::Error::new_spanned(&meta.path, "clap argument `id` must be a string literal")
    })?;
    *existing_id = Some(lit);
    Ok(())
}

pub(crate) fn clap_arg_id_from_attribute(
    attr: &syn::Attribute,
    existing_id: &mut Option<syn::LitStr>,
) -> syn::Result<()> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(());
    };

    list.parse_nested_meta(|meta| parse_id_from_meta(&meta, existing_id))
}

pub(crate) fn clap_arg_id(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut arg_id: Option<syn::LitStr> = None;
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        clap_arg_id_from_attribute(attr, &mut arg_id)?;
    }
    Ok(arg_id.map(|lit| lit.value()))
}

#[derive(Clone)]
pub(crate) enum ClapInferredDefault {
    Value(Expr),
    ValueT(Expr),
    ValuesT(Expr),
}

#[derive(Clone, Copy)]
enum ClapDefaultKind {
    Value,
    ValueT,
    ValuesT,
    Other,
}

fn assign_default_expr(
    meta: &syn::meta::ParseNestedMeta<'_>,
    default_slot: &mut Option<ClapInferredDefault>,
    parsed_expr: ClapInferredDefault,
) -> syn::Result<()> {
    if default_slot.is_some() {
        return Err(syn::Error::new_spanned(
            &meta.path,
            "duplicate clap default override",
        ));
    }
    *default_slot = Some(parsed_expr);
    Ok(())
}

fn classify_default_kind(meta: &syn::meta::ParseNestedMeta<'_>) -> ClapDefaultKind {
    if meta.path.is_ident("default_value") {
        return ClapDefaultKind::Value;
    }
    if meta.path.is_ident("default_value_t") {
        return ClapDefaultKind::ValueT;
    }
    if meta.path.is_ident("default_values_t") {
        return ClapDefaultKind::ValuesT;
    }
    ClapDefaultKind::Other
}

fn parse_default_expr(
    meta: &syn::meta::ParseNestedMeta<'_>,
    kind: ClapDefaultKind,
) -> syn::Result<Option<ClapInferredDefault>> {
    let parsed = match kind {
        ClapDefaultKind::Value => ClapInferredDefault::Value(meta.value()?.parse::<Expr>()?),
        ClapDefaultKind::ValueT => ClapInferredDefault::ValueT(meta.value()?.parse::<Expr>()?),
        ClapDefaultKind::ValuesT => ClapInferredDefault::ValuesT(meta.value()?.parse::<Expr>()?),
        ClapDefaultKind::Other => return Ok(None),
    };
    Ok(Some(parsed))
}

/// Parses clap default-related keys from a nested meta item.
///
/// Recognised keys:
///
/// - `default_value = "..."`
/// - `default_value_t = <expr>`
/// - `default_values_t = <expr>`
///
/// Duplicate defaults (including mixed forms) produce a compile-time error.
pub(crate) fn parse_default_from_meta(
    meta: &syn::meta::ParseNestedMeta<'_>,
    existing_default: &mut Option<ClapInferredDefault>,
) -> syn::Result<()> {
    if let Some(parsed) = parse_default_expr(meta, classify_default_kind(meta))? {
        return assign_default_expr(meta, existing_default, parsed);
    }

    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        let _: Expr = value.parse()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

pub(crate) fn clap_default_value_from_attribute(
    attr: &syn::Attribute,
    existing_default: &mut Option<ClapInferredDefault>,
) -> syn::Result<()> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(());
    };

    list.parse_nested_meta(|meta| parse_default_from_meta(&meta, existing_default))
}

/// Returns the typed default expression inferred from clap attributes, if any.
///
/// The generated defaults struct consumes these inferred values and
/// materializes field-level defaults during code generation.
pub(crate) fn clap_default_value(field: &syn::Field) -> syn::Result<Option<ClapInferredDefault>> {
    let mut default_expr: Option<ClapInferredDefault> = None;
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        clap_default_value_from_attribute(attr, &mut default_expr)?;
    }
    Ok(default_expr)
}
