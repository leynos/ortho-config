//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file and
//! environment variables. Environment variable names are automatically mapped
//! from `snake_case` field names to `UPPER_SNAKE_CASE`.

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
                use figment::{Figment, providers::{Toml, Env}};

                Figment::new()
                    .merge(Toml::file("config.toml"))
                    .merge(Env::raw()
                        .map(|k| k.to_ascii_uppercase())
                        .split("__"))
                    .extract()
                    .map_err(ortho_config::OrthoError::Gathering)
            }
        }
    };

    TokenStream::from(expanded)
}
