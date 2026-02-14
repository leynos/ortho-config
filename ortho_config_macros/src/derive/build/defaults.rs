//! Default struct helpers for the derive macro.
//!
//! These functions materialise the intermediate defaults struct that collects
//! per-field values before layered configuration is merged.

use quote::quote;

use crate::derive::parse::{ClapInferredDefault, FieldAttrs};

use super::cli::option_type_tokens;

fn require_named_field(field: &syn::Field) -> Result<&syn::Ident, proc_macro2::TokenStream> {
    field.ident.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(field, "OrthoConfig defaults structs require named fields")
            .to_compile_error()
    })
}

pub(crate) fn build_default_struct_fields(fields: &[syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|f| {
            let name = match require_named_field(f) {
                Ok(ident) => ident,
                Err(err) => return err,
            };
            let ty = option_type_tokens(&f.ty);
            quote! {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #name: #ty
            }
        })
        .collect()
}

pub(crate) fn build_default_struct_init(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .zip(field_attrs.iter())
        .map(|(f, attr)| {
            let name = match require_named_field(f) {
                Ok(ident) => ident,
                Err(err) => return err,
            };
            let default_expr = attr.default.as_ref().map_or_else(
                || inferred_default_expr(attr.inferred_clap_default.as_ref()),
                |expr| Some(quote! { #expr }),
            );
            default_expr.map_or_else(
                || quote! { #name: None },
                |expr| quote! { #name: Some(#expr) },
            )
        })
        .collect()
}

fn inferred_default_expr(
    inferred: Option<&ClapInferredDefault>,
) -> Option<proc_macro2::TokenStream> {
    inferred.map(|default| match default {
        ClapInferredDefault::Value(expr) => {
            // This variant is rejected in parsing for `cli_default_as_absent`.
            quote! { #expr }
        }
        // The expression is emitted verbatim because clap requires
        // `default_value_t` to evaluate to the field type.  Wrapping in
        // `Into::into` would break unsuffixed integer literals (e.g. `8`)
        // whose type is otherwise inferred from the surrounding context.
        ClapInferredDefault::ValueT(expr) => quote! { #expr },
        ClapInferredDefault::ValuesT(expr) => quote! {
            ::std::iter::IntoIterator::into_iter(#expr)
                .collect::<::std::vec::Vec<_>>()
        },
    })
}
