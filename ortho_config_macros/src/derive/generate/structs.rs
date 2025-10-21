//! Struct generation helpers for the `OrthoConfig` derive macro.
//!
//! These utilities emit hidden support structs such as the CLI argument parser
//! and defaults storage. Keeping them in a dedicated module keeps the derive
//! entrypoint concise while allowing focused unit tests for each generator.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::MacroComponents;

/// Generate a struct definition with optional field tokens.
///
/// # Examples
///
/// ```ignore
/// use quote::quote;
/// use syn::parse_str;
///
/// let ident = parse_str("Example").expect("ident");
/// let fields = vec![quote! { pub value: u32 }];
/// let attrs = quote! { #[derive(Default)] };
/// let tokens = ortho_config_macros::derive::generate::structs::generate_struct(
///     &ident,
///     &fields,
///     &attrs,
/// );
/// assert!(tokens.to_string().contains("struct Example"));
/// ```
pub(crate) fn generate_struct(
    ident: &Ident,
    fields: &[TokenStream],
    attributes: &TokenStream,
) -> TokenStream {
    if fields.is_empty() {
        quote! {
            #attributes
            struct #ident {}
        }
    } else {
        quote! {
            #attributes
            struct #ident {
                #( #fields, )*
            }
        }
    }
}

/// Generate the hidden `clap::Parser` struct.
pub(crate) fn generate_cli_struct(components: &MacroComponents) -> TokenStream {
    let MacroComponents {
        cli_ident,
        cli_struct_fields,
        ..
    } = components;
    generate_struct(
        cli_ident,
        cli_struct_fields,
        &quote! { #[derive(clap::Parser, serde::Serialize, Default)] },
    )
}

/// Generate the struct used to store default values.
pub(crate) fn generate_defaults_struct(components: &MacroComponents) -> TokenStream {
    let MacroComponents {
        defaults_ident,
        default_struct_fields,
        ..
    } = components;
    generate_struct(
        defaults_ident,
        default_struct_fields,
        &quote! { #[derive(serde::Serialize)] },
    )
}
