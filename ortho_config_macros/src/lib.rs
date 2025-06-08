//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file,
//! environment variables, and now command-line arguments via `clap`. CLI flag
//! names are automatically generated from `snake_case` field names using the
//! `kebab-case` convention.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derive macro for [`ortho_config::OrthoConfig`].
#[proc_macro_derive(OrthoConfig)]
pub fn derive_ortho_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(named) => named.named,
            _ => {
                return syn::Error::new_spanned(
                    data.struct_token,
                    "OrthoConfig requires named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(ident, "OrthoConfig can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let cli_ident = format_ident!("__{}Cli", ident);
    let cli_mod = format_ident!("__{}CliMod", ident);

    let cli_fields = fields.iter().map(|f| {
        let name = f.ident.as_ref().expect("named field");
        let ty = &f.ty;
        quote! {
            #[arg(long, required = false)]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub(super) #name: ::core::option::Option<#ty>
        }
    });

    let expanded = quote! {
        mod #cli_mod {
            #[derive(clap::Parser, serde::Serialize)]
            #[command(rename_all = "kebab-case")]
            pub(super) struct #cli_ident {
                #( #cli_fields, )*
            }
        }

        impl #ident {
            #[allow(dead_code)]
            fn load_from_iter<I>(args: I) -> Result<Self, ortho_config::OrthoError>
            where
                I: IntoIterator,
                I::Item: AsRef<::std::ffi::OsStr>,
            {
                use clap::Parser as _;
                use figment::{Figment, providers::{Toml, Env, Serialized, Format}, Profile};
                use uncased::Uncased;

                let cli = #cli_mod::#cli_ident::try_parse_from(args)
                    .map_err(ortho_config::OrthoError::CliParsing)?;

                Figment::new()
                    .merge(Toml::file("config.toml"))
                    .merge(Env::raw()
                        .map(|k| Uncased::new(k.as_str().to_ascii_uppercase()))
                        .split("__"))
                    .merge(Serialized::from(cli, Profile::Default))
                    .extract()
                    .map_err(ortho_config::OrthoError::Gathering)
            }
        }

        impl ortho_config::OrthoConfig for #ident {
            fn load() -> Result<Self, ortho_config::OrthoError> {
                Self::load_from_iter(::std::env::args_os())
            }
        }
    };

    TokenStream::from(expanded)
}
