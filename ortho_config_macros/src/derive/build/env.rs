//! Environment discovery helpers for the derive macro.
//!
//! The functions here compute environment variable names, dotfile defaults, and
//! providers that back the generated `load` implementation.

use heck::ToSnakeCase;
use quote::quote;
use syn::Ident;

use crate::derive::parse::StructAttrs;

pub(crate) fn build_env_provider(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    if let Some(prefix) = &struct_attrs.prefix {
        quote! { ortho_config::CsvEnv::prefixed(#prefix) }
    } else {
        quote! { ortho_config::CsvEnv::raw() }
    }
}

pub(crate) fn compute_config_env_var(struct_attrs: &StructAttrs) -> String {
    struct_attrs.prefix.as_deref().map_or_else(
        || String::from("CONFIG_PATH"),
        |prefix| format!("{prefix}CONFIG_PATH"),
    )
}

pub(crate) fn build_config_env_var(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    let var = compute_config_env_var(struct_attrs);
    quote! { #var }
}

pub(crate) fn compute_dotfile_name(struct_attrs: &StructAttrs) -> String {
    if let Some(prefix) = &struct_attrs.prefix {
        let base = prefix.trim_end_matches('_').to_ascii_lowercase();
        format!(".{base}.toml")
    } else {
        String::from(".config.toml")
    }
}

pub(crate) fn default_app_name(struct_attrs: &StructAttrs, ident: &Ident) -> String {
    if let Some(prefix) = &struct_attrs.prefix {
        let normalised = prefix.trim_end_matches('_').to_ascii_lowercase();
        if !normalised.is_empty() {
            return normalised;
        }
    }
    ident.to_string().to_snake_case()
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "tests panic to surface configuration mistakes"
    )]
    use super::*;

    fn demo_input() -> (
        Vec<syn::Field>,
        Vec<crate::derive::parse::FieldAttrs>,
        StructAttrs,
    ) {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[ortho_config(prefix = "CFG_")]
            struct Demo {
                #[ortho_config(cli_long = "opt", cli_short = 'o', default = 5)]
                field1: Option<u32>,
                #[ortho_config(merge_strategy = "append")]
                field2: Vec<String>,
            }
        };
        let (_, fields, struct_attrs, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        (fields, field_attrs, struct_attrs)
    }

    #[test]
    fn env_provider_tokens() {
        let (_, _, struct_attrs) = demo_input();
        let ts = build_env_provider(&struct_attrs);
        assert_eq!(
            ts.to_string(),
            "ortho_config :: CsvEnv :: prefixed (\"CFG_\")",
        );
    }
}
