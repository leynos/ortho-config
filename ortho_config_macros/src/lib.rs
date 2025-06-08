//! Procedural macros for `ortho_config`.
//!
//! At this stage the [`OrthoConfig`] derive generates an empty `load` method
//! which will be fleshed out in later versions.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Derive macro for [`ortho_config::OrthoConfig`].
#[proc_macro_derive(OrthoConfig)]
pub fn derive_ortho_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;

    let expanded = quote! {
        impl ortho_config::OrthoConfig for #ident {
            fn load() -> Result<Self, ortho_config::OrthoError> {
                todo!()
            }
        }
    };

    TokenStream::from(expanded)
}
