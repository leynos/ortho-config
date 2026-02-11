//! Parsing helpers for clap field attributes.
//!
//! These helpers extract metadata from `#[arg(...)]` and `#[clap(...)]`
//! attributes without taking a dependency on clap itself.

use syn::spanned::Spanned;
use syn::{Expr, Type};

use super::option_inner;

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

fn typed_expr_from_default_value(
    raw_expr: &Expr,
    default_target_ty: &Type,
    field_name_lit: &syn::LitStr,
) -> Expr {
    syn::parse_quote_spanned! { raw_expr.span() =>
        {
            let __ortho_default_value = #raw_expr;
            let __ortho_default_value = ::core::convert::AsRef::<str>::as_ref(
                &__ortho_default_value,
            );
            match <#default_target_ty as ::core::str::FromStr>::from_str(__ortho_default_value) {
                ::core::result::Result::Ok(value) => value,
                ::core::result::Result::Err(_) => {
                    panic!(
                        concat!(
                            "invalid clap `default_value` for field `",
                            #field_name_lit,
                            "`; consider using `default_value_t` for typed defaults"
                        )
                    )
                }
            }
        }
    }
}

fn typed_expr_from_default_value_t(raw_expr: &Expr) -> Expr {
    syn::parse_quote_spanned! { raw_expr.span() =>
        ::core::convert::Into::into(#raw_expr)
    }
}

fn typed_expr_from_default_values_t(raw_expr: &Expr) -> Expr {
    syn::parse_quote_spanned! { raw_expr.span() =>
        ::std::iter::IntoIterator::into_iter(#raw_expr)
            .map(::core::convert::Into::into)
            .collect::<::std::vec::Vec<_>>()
    }
}

fn assign_default_expr(
    meta: &syn::meta::ParseNestedMeta<'_>,
    default_slot: &mut Option<Expr>,
    parsed_expr: Expr,
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
    existing_default: &mut Option<Expr>,
    default_target_ty: &Type,
    field_name_lit: &syn::LitStr,
) -> syn::Result<()> {
    if meta.path.is_ident("default_value") {
        let value = meta.value()?;
        let raw_expr = value.parse::<Expr>()?;
        let parsed = typed_expr_from_default_value(&raw_expr, default_target_ty, field_name_lit);
        return assign_default_expr(meta, existing_default, parsed);
    }

    if meta.path.is_ident("default_value_t") {
        let value = meta.value()?;
        let raw_expr = value.parse::<Expr>()?;
        let parsed = typed_expr_from_default_value_t(&raw_expr);
        return assign_default_expr(meta, existing_default, parsed);
    }

    if meta.path.is_ident("default_values_t") {
        let value = meta.value()?;
        let raw_expr = value.parse::<Expr>()?;
        let parsed = typed_expr_from_default_values_t(&raw_expr);
        return assign_default_expr(meta, existing_default, parsed);
    }

    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        let _: Expr = value.parse()?;
    }
    Ok(())
}

pub(crate) fn clap_default_value_from_attribute(
    attr: &syn::Attribute,
    existing_default: &mut Option<Expr>,
    default_target_ty: &Type,
    field_name_lit: &syn::LitStr,
) -> syn::Result<()> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(());
    };

    list.parse_nested_meta(|meta| {
        parse_default_from_meta(&meta, existing_default, default_target_ty, field_name_lit)
    })
}

/// Returns the typed default expression inferred from clap attributes, if any.
///
/// The default target type matches the generated defaults struct:
/// - For `T` fields, infer `T`
/// - For `Option<T>` fields, infer `T`
pub(crate) fn clap_default_value(field: &syn::Field) -> syn::Result<Option<Expr>> {
    let mut default_expr: Option<Expr> = None;
    let default_target_ty = option_inner(&field.ty).unwrap_or(&field.ty);
    let field_name = field.ident.as_ref().map_or_else(
        || String::from("<unnamed>"),
        std::string::ToString::to_string,
    );
    let field_name_lit = syn::LitStr::new(&field_name, proc_macro2::Span::call_site());

    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        clap_default_value_from_attribute(
            attr,
            &mut default_expr,
            default_target_ty,
            &field_name_lit,
        )?;
    }
    Ok(default_expr)
}
