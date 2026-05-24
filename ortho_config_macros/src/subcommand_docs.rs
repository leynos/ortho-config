//! Generates `OrthoConfigSubcommandDocs` implementations for command enums.
//!
//! This module powers the derive macro by validating the input enum and
//! emitting one documentation metadata entry per tuple variant.

use heck::ToKebabCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::derive::parse::clap_variant_name;

fn parse_crate_path(attrs: &[syn::Attribute]) -> syn::Result<Option<syn::Path>> {
    let mut crate_path = None;
    for attr in attrs {
        if !attr.path().is_ident("ortho_config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("crate") {
                return Err(meta.error("unsupported ortho_config option on enum"));
            }
            if crate_path.is_some() {
                return Err(meta.error("duplicate `crate` attribute"));
            }
            crate_path = Some(crate::derive::parse::lit_crate_path(&meta)?);
            Ok(())
        })?;
    }
    Ok(crate_path)
}

fn single_tuple_field<'a>(
    variant_ident: &syn::Ident,
    fields: &'a syn::Fields,
) -> syn::Result<&'a syn::Type> {
    match fields {
        syn::Fields::Unnamed(unnamed_fields) => single_unnamed_type(variant_ident, unnamed_fields),
        syn::Fields::Named(_) => Err(syn::Error::new_spanned(
            variant_ident,
            "named-field variants are not supported; use tuple variants like Variant(Args)",
        )),
        syn::Fields::Unit => Err(syn::Error::new_spanned(
            variant_ident,
            "unit variants are not supported; use Variant(Args)",
        )),
    }
}

fn single_unnamed_type<'a>(
    variant_ident: &syn::Ident,
    unnamed_fields: &'a syn::FieldsUnnamed,
) -> syn::Result<&'a syn::Type> {
    let mut fields = unnamed_fields.unnamed.iter();
    let Some(field) = fields.next() else {
        return Err(syn::Error::new_spanned(
            variant_ident,
            "unit variants are not supported; use Variant(Args)",
        ));
    };
    if fields.next().is_some() {
        return Err(syn::Error::new_spanned(
            variant_ident,
            "tuple variants must contain exactly one field",
        ));
    }
    Ok(&field.ty)
}

fn command_label(variant: &syn::Variant) -> syn::Result<syn::LitStr> {
    Ok(clap_variant_name(variant)?.unwrap_or_else(|| {
        syn::LitStr::new(
            &variant.ident.to_string().to_kebab_case(),
            variant.ident.span(),
        )
    }))
}

fn metadata_expr(variant: &syn::Variant, krate: &TokenStream) -> syn::Result<TokenStream> {
    let args_ty = single_tuple_field(&variant.ident, &variant.fields)?;
    let label = command_label(variant)?;
    Ok(quote! {
        {
            let mut metadata =
                <#args_ty as #krate::docs::OrthoConfigDocs>::get_doc_metadata();
            metadata.app_name = #label.to_string();
            metadata.about_id = format!("{}.about", metadata.app_name);
            metadata
        }
    })
}

/// Build the `OrthoConfigSubcommandDocs` implementation for the input enum.
pub(crate) fn derive_subcommand_docs(input: DeriveInput) -> syn::Result<TokenStream> {
    let crate_path = parse_crate_path(&input.attrs)?;
    let krate = crate::derive::crate_path::resolve(crate_path.as_ref());

    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let syn::Data::Enum(enum_data) = input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "OrthoConfigSubcommandDocs can only be derived for enums",
        ));
    };

    let metadata_exprs = enum_data
        .variants
        .iter()
        .map(|variant| metadata_expr(variant, &krate))
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        impl #impl_generics #krate::docs::OrthoConfigSubcommandDocs for #ident #ty_generics #where_clause {
            fn get_subcommand_doc_metadata() -> Vec<#krate::docs::DocMetadata> {
                vec![#(#metadata_exprs),*]
            }
        }
    })
}
