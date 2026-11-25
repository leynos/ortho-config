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
///
/// An explicit `#[ortho_config(default = ...)]` wins. Without one, a clap
/// `default_value` reaches the documentation IR only when
/// `cli_default_as_absent` asked the derive to model it; otherwise the literal
/// stays clap-only and no `DefaultValue` is emitted.
pub(super) fn default_tokens(attrs: &FieldAttrs, krate: &TokenStream) -> TokenStream {
    let display_str = attrs.default.as_ref().map_or_else(
        || {
            attrs
                .inferred_clap_default
                .as_ref()
                .map(|inferred| match inferred {
                    crate::derive::parse::ClapInferredDefault::Value(default) => {
                        default.value.to_token_stream().to_string()
                    }
                    crate::derive::parse::ClapInferredDefault::ValueT(expr)
                    | crate::derive::parse::ClapInferredDefault::ValuesT(expr) => {
                        expr.to_token_stream().to_string()
                    }
                })
        },
        |expr| Some(expr.to_token_stream().to_string()),
    );

    build_optional_doc_metadata(
        display_str.as_deref(),
        &quote! { #krate::docs::DefaultValue },
        "display",
    )
}

/// Generates tokens for the deprecation metadata.
pub(super) fn deprecated_tokens(attrs: &FieldAttrs, krate: &TokenStream) -> TokenStream {
    build_optional_doc_metadata(
        attrs.doc.deprecated_note_id.as_deref(),
        &quote! { #krate::docs::Deprecation },
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

#[cfg(test)]
mod tests {
    //! Unit tests for default-value token generation.

    use super::*;
    use crate::derive::parse::{ClapDefaultValue, ClapDefaultValueShape, ClapInferredDefault};
    use syn::parse_quote;

    /// Attributes for a protected field whose default comes from clap.
    fn attrs_with_inferred_default() -> FieldAttrs {
        FieldAttrs {
            inferred_clap_default: Some(ClapInferredDefault::Value(Box::new(ClapDefaultValue {
                value: parse_quote!("INFERRED-DEFAULT-SENTINEL"),
                value_parser: None,
                value_enum: false,
                value_delimiter: None,
                ignore_case: None,
                leaf_type: parse_quote!(String),
                shape: ClapDefaultValueShape::Scalar,
            }))),
            ..FieldAttrs::default()
        }
    }

    #[test]
    fn inferred_clap_default_reaches_the_documentation_default() {
        let rendered = default_tokens(&attrs_with_inferred_default(), &quote!(krate)).to_string();
        assert!(
            rendered.contains("INFERRED-DEFAULT-SENTINEL"),
            "the inferred clap default must reach the documentation default before projection: {rendered}"
        );
    }

    #[test]
    fn no_default_emits_none() {
        let rendered = default_tokens(&FieldAttrs::default(), &quote!(krate)).to_string();
        assert_eq!(
            rendered, "None",
            "a field with no default must emit no documentation default"
        );
    }

    #[test]
    fn explicit_default_wins_over_the_inferred_one() {
        let mut attrs = attrs_with_inferred_default();
        attrs.default = Some(parse_quote!(String::from("EXPLICIT-DEFAULT")));
        let rendered = default_tokens(&attrs, &quote!(krate)).to_string();
        assert!(
            rendered.contains("EXPLICIT-DEFAULT"),
            "an explicit default must win: {rendered}"
        );
        assert!(
            !rendered.contains("INFERRED-DEFAULT-SENTINEL"),
            "the clap literal must not also be emitted: {rendered}"
        );
    }
}
