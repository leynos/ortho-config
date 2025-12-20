//! Generates `SelectedSubcommandMerge` implementations for subcommand enums.
//!
//! This module powers the derive macro by validating the input enum and
//! emitting the per-variant merge logic.

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

fn merge_expr(uses_matches: bool, selected_label: &syn::LitStr) -> proc_macro2::TokenStream {
    if uses_matches {
        quote! {
            {
                let subcommand_matches = matches
                    .subcommand()
                    .map(|(_, subcommand_matches)| subcommand_matches)
                    .ok_or_else(|| {
                        ortho_config::SelectedSubcommandMergeError::MissingSubcommandMatches {
                            selected: #selected_label,
                        }
                    })?;
                ortho_config::SubcmdConfigMerge::load_and_merge_with_matches(&args, subcommand_matches)
                    .map_err(ortho_config::SelectedSubcommandMergeError::from)?
            }
        }
    } else {
        quote! {
            ortho_config::SubcmdConfigMerge::load_and_merge(&args)
                .map_err(ortho_config::SelectedSubcommandMergeError::from)?
        }
    }
}

fn build_arm(
    variant_ident: &syn::Ident,
    merge_expr: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        Self::#variant_ident(args) => {
            let merged = #merge_expr;
            Ok(Self::#variant_ident(merged))
        }
    }
}

/// Build the `SelectedSubcommandMerge` implementation for the input enum.
pub(crate) fn derive_selected_subcommand_merge(
    input: DeriveInput,
) -> syn::Result<proc_macro2::TokenStream> {
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
        let variant_ident = variant.ident;
        let selected_label = syn::LitStr::new(&variant_ident.to_string(), variant_ident.span());

        validate_tuple_variant(&variant_ident, &variant.fields)?;
        let merge_tokens = merge_expr(uses_matches, &selected_label);
        arms.push(build_arm(&variant_ident, &merge_tokens));
    }

    Ok(quote! {
        impl #impl_generics ortho_config::SelectedSubcommandMerge for #ident #ty_generics #where_clause {
            fn load_and_merge_selected(
                self,
                matches: &clap::ArgMatches,
            ) -> std::result::Result<Self, ortho_config::SelectedSubcommandMergeError> {
                match self {
                    #(#arms)*
                }
            }
        }
    })
}
