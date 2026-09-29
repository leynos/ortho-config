//! Environment discovery helpers for the derive macro.
//!
//! The functions here compute environment variable names, dotfile defaults, and
//! providers that back the generated `load` implementation.

use heck::ToSnakeCase;
use quote::quote;
use syn::Ident;

use crate::derive::parse::StructAttrs;

/// Builds the CSV environment provider used by generated loading code.
///
/// Both prefixed and unprefixed forms uppercase keys and split nested names on
/// `__`, matching the library's environment hierarchy convention.
pub(crate) fn build_env_provider(
    struct_attrs: &StructAttrs,
    krate: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    struct_attrs.prefix.as_ref().map_or_else(
        || quote! { #krate::CsvEnv::raw().uppercase(true).split("__") },
        |prefix| quote! { #krate::CsvEnv::prefixed(#prefix).uppercase(true).split("__") },
    )
}

/// Selects the config-path environment key, preserving the prefix spelling
/// while falling back to `CONFIG_PATH` when no prefix is configured.
pub(crate) fn compute_config_env_var(struct_attrs: &StructAttrs) -> String {
    struct_attrs.prefix.as_deref().map_or_else(
        || String::from("CONFIG_PATH"),
        |prefix| format!("{prefix}CONFIG_PATH"),
    )
}

/// Converts the resolved config-path key into a literal expression for the
/// generated load implementation.
pub(crate) fn build_config_env_var(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    let var = compute_config_env_var(struct_attrs);
    quote! { #var }
}

/// Derives the default dotfile name from the prefix after removing trailing
/// separators and normalizing its case.
pub(crate) fn compute_dotfile_name(struct_attrs: &StructAttrs) -> String {
    struct_attrs.prefix.as_ref().map_or_else(
        || String::from(".config.toml"),
        |prefix| {
            let base = prefix.trim_end_matches('_').to_ascii_lowercase();
            format!(".{base}.toml")
        },
    )
}

/// Uses the normalized prefix as the discovery application name, falling back
/// to the configuration type's snake-case name when the prefix is empty.
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
    //! Unit tests for environment-variable token generation.

    use super::*;
    use anyhow::{Result, anyhow, ensure};

    fn demo_input() -> Result<(
        Vec<syn::Field>,
        Vec<crate::derive::parse::FieldAttrs>,
        StructAttrs,
    )> {
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
            crate::derive::parse::parse_input(&input).map_err(|err| anyhow!(err))?;
        Ok((fields, field_attrs, struct_attrs))
    }

    #[test]
    fn env_provider_tokens() -> Result<()> {
        let (_, _, struct_attrs) = demo_input()?;
        let krate = quote! { ortho_config };
        let ts = build_env_provider(&struct_attrs, &krate);
        ensure!(
            ts.to_string()
                == "ortho_config :: CsvEnv :: prefixed (\"CFG_\") . uppercase (true) . split (\"__\")",
            "unexpected env provider tokens"
        );
        Ok(())
    }
}
