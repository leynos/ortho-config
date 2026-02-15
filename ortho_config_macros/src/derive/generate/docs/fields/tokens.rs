//! Token generation helpers for field documentation metadata.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::derive::parse::FieldAttrs;

use super::value_types::{ValueTypeModel, enum_variants};

/// Helper to build optional metadata tokens with a single string field.
pub(super) fn build_optional_doc_metadata(
    value: Option<&str>,
    struct_path: &TokenStream,
    field_name: &str,
) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |s| {
            let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
            let field_ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
            quote! {
                Some(#struct_path {
                    #field_ident: String::from(#lit),
                })
            }
        },
    )
}

/// Generates tokens for the default value metadata.
pub(super) fn default_tokens(attrs: &FieldAttrs) -> TokenStream {
    let display_str = attrs.default.as_ref().map_or_else(
        || {
            attrs
                .inferred_clap_default
                .as_ref()
                .map(|inferred| match inferred {
                    crate::derive::parse::ClapInferredDefault::Value(expr)
                    | crate::derive::parse::ClapInferredDefault::ValueT(expr)
                    | crate::derive::parse::ClapInferredDefault::ValuesT(expr) => {
                        expr.to_token_stream().to_string()
                    }
                })
        },
        |expr| Some(expr.to_token_stream().to_string()),
    );

    build_optional_doc_metadata(
        display_str.as_deref(),
        &quote! { ortho_config::docs::DefaultValue },
        "display",
    )
}

/// Generates tokens for the deprecation metadata.
pub(super) fn deprecated_tokens(attrs: &FieldAttrs) -> TokenStream {
    build_optional_doc_metadata(
        attrs.doc.deprecated_note_id.as_deref(),
        &quote! { ortho_config::docs::Deprecation },
        "note_id",
    )
}

/// Generates tokens for enum possible values.
pub(super) fn build_possible_values(value_type: Option<&ValueTypeModel>) -> Vec<TokenStream> {
    value_type
        .and_then(enum_variants)
        .map_or_else(Vec::new, |variants| {
            variants
                .iter()
                .map(|value| {
                    let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
                    quote! { String::from(#lit) }
                })
                .collect::<Vec<_>>()
        })
}
