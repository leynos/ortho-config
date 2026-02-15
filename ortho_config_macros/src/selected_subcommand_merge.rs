//! Generates `SelectedSubcommandMerge` implementations for subcommand enums.
//!
//! This module powers the derive macro by validating the input enum and
//! emitting the per-variant merge logic.

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

fn variant_uses_matches(variant: &syn::Variant) -> syn::Result<bool> {
    let mut uses = false;
    for attr in &variant.attrs {
        if !attr.path().is_ident("ortho_subcommand") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("with_matches") {
                uses = true;
                return Ok(());
            }
            Err(meta.error("unsupported ortho_subcommand option"))
        })?;
    }
    Ok(uses)
}

fn clap_variant_name(variant: &syn::Variant) -> syn::Result<Option<syn::LitStr>> {
    let mut name = None;
    for attr in &variant.attrs {
        let is_command = attr.path().is_ident("command") || attr.path().is_ident("clap");
        if !is_command {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                name = Some(lit);
                return Ok(());
            }
            Ok(())
        })?;
    }
    Ok(name)
}

fn validate_tuple_variant(variant_ident: &syn::Ident, fields: &syn::Fields) -> syn::Result<()> {
    match fields {
        syn::Fields::Unnamed(unnamed_fields) if unnamed_fields.unnamed.len() == 1 => Ok(()),
        syn::Fields::Named(_) => Err(syn::Error::new_spanned(
            variant_ident,
            "named-field variants are not supported; use tuple variants like Variant(Args)",
        )),
        syn::Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            variant_ident,
            "tuple variants must contain exactly one field",
        )),
        syn::Fields::Unit => Err(syn::Error::new_spanned(
            variant_ident,
            "unit variants are not supported; use Variant(Args)",
        )),
    }
}

/// Extract `crate = "..."` from `#[ortho_config(...)]` attributes.
fn parse_crate_path(attrs: &[syn::Attribute]) -> syn::Result<Option<syn::Path>> {
    let mut crate_path = None;
    for attr in attrs {
        if !attr.path().is_ident("ortho_config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                let path: syn::Path =
                    syn::parse_str(&lit.value()).map_err(|e| syn::Error::new(lit.span(), e))?;
                crate_path = Some(path);
                return Ok(());
            }
            Err(meta.error("unsupported ortho_config option on enum"))
        })?;
    }
    Ok(crate_path)
}

fn merge_expr(
    uses_matches: bool,
    selected_label: &syn::LitStr,
    krate: &TokenStream,
) -> TokenStream {
    if uses_matches {
        quote! {
            {
                let subcommand_matches = matches
                    .subcommand()
                    .map(|(_, subcommand_matches)| subcommand_matches)
                    .ok_or_else(|| {
                        #krate::SelectedSubcommandMergeError::MissingSubcommandMatches {
                            selected: #selected_label,
                        }
                    })?;
                #krate::SubcmdConfigMerge::load_and_merge_with_matches(&args, subcommand_matches)
                    .map_err(#krate::SelectedSubcommandMergeError::from)?
            }
        }
    } else {
        quote! {
            #krate::SubcmdConfigMerge::load_and_merge(&args)
                .map_err(#krate::SelectedSubcommandMergeError::from)?
        }
    }
}

fn build_arm(variant_ident: &syn::Ident, merge_expr: &TokenStream) -> TokenStream {
    quote! {
        Self::#variant_ident(args) => {
            let merged = #merge_expr;
            Ok(Self::#variant_ident(merged))
        }
    }
}

/// Build the `SelectedSubcommandMerge` implementation for the input enum.
pub(crate) fn derive_selected_subcommand_merge(input: DeriveInput) -> syn::Result<TokenStream> {
    let crate_path = parse_crate_path(&input.attrs)?;
    let krate = crate::derive::crate_path::resolve(crate_path.as_ref());

    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let syn::Data::Enum(enum_data) = input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "SelectedSubcommandMerge can only be derived for enums",
        ));
    };

    let mut arms = Vec::new();
    for variant in enum_data.variants {
        let uses_matches = variant_uses_matches(&variant)?;
        let selected_label = clap_variant_name(&variant)?
            .unwrap_or_else(|| syn::LitStr::new(&variant.ident.to_string(), variant.ident.span()));

        let variant_ident = variant.ident;
        validate_tuple_variant(&variant_ident, &variant.fields)?;
        let merge_tokens = merge_expr(uses_matches, &selected_label, &krate);
        arms.push(build_arm(&variant_ident, &merge_tokens));
    }

    Ok(quote! {
        impl #impl_generics #krate::SelectedSubcommandMerge for #ident #ty_generics #where_clause {
            fn load_and_merge_selected(
                self,
                matches: &clap::ArgMatches,
            ) -> std::result::Result<Self, #krate::SelectedSubcommandMergeError> {
                match self {
                    #(#arms)*
                }
            }
        }
    })
}
