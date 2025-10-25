//! Default struct helpers for the derive macro.
//!
//! These functions materialise the intermediate defaults struct that collects
//! per-field values before layered configuration is merged.

use quote::quote;

use crate::derive::parse::FieldAttrs;

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
            attr.default.as_ref().map_or_else(
                || quote! { #name: None },
                |expr| quote! { #name: Some(#expr) },
            )
        })
        .collect()
}
