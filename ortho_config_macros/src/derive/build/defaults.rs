//! Default struct helpers for the derive macro.
//!
//! These functions materialise the intermediate defaults struct that collects
//! per-field values before layered configuration is merged.

use quote::quote;

use crate::derive::parse::FieldAttrs;

use super::cli::option_type_tokens;

pub(crate) fn build_default_struct_fields(fields: &[syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|f| {
            let Some(name) = f.ident.as_ref() else {
                return syn::Error::new_spanned(
                    f,
                    "OrthoConfig defaults structs require named fields",
                )
                .to_compile_error();
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
            let Some(name) = f.ident.as_ref() else {
                return syn::Error::new_spanned(
                    f,
                    "OrthoConfig defaults structs require named fields",
                )
                .to_compile_error();
            };
            if let Some(expr) = &attr.default {
                quote! { #name: Some(#expr) }
            } else {
                quote! { #name: None }
            }
        })
        .collect()
}
