//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file,
//! environment variables, and now command-line arguments via `clap`. CLI flag
//! names are automatically generated from `snake_case` field names using the
//! `kebab-case` convention.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type, parse_macro_input};

fn option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

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
    let cli_pub_ident = format_ident!("{}Cli", ident);

    let cli_fields = fields.iter().map(|f| {
        let name = f.ident.as_ref().expect("named field");
        let ty = &f.ty;
        let inner = option_inner(ty);
        let cli_ty = if let Some(inner) = inner {
            quote! { Option<#inner> }
        } else {
            quote! { Option<#ty> }
        };
        quote! {
            #[arg(long, required = false)]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub(super) #name: #cli_ty
        }
    });

    let expanded = quote! {
        mod #cli_mod {
            use std::option::Option as Option;
            #[derive(clap::Parser, serde::Serialize)]
            #[command(rename_all = "kebab-case")]
            pub struct #cli_ident {
                #( #cli_fields, )*
            }
        }

        pub use #cli_mod::#cli_ident as #cli_pub_ident;

        impl #ident {
            #[allow(dead_code)]
            pub fn load_from_iter<I, S>(args: I) -> Result<Self, ortho_config::OrthoError>
            where
                I: IntoIterator<Item = S>,
                S: AsRef<std::ffi::OsStr>,
            {
                use clap::Parser as _;
                use figment::{Figment, providers::{Toml, Env, Serialized, Format}, Profile};
                use uncased::Uncased;

                let cli_args: Vec<std::ffi::OsString> = args
                    .into_iter()
                    .map(|a| a.as_ref().to_os_string())
                    .collect();

                let cli = #cli_mod::#cli_ident::try_parse_from(cli_args)
                    .map_err(ortho_config::OrthoError::CliParsing)?;

                let cfg_path = std::env::var("CONFIG_PATH")
                    .unwrap_or_else(|_| "config.toml".to_string());

                let mut fig = Figment::new();
                if std::path::Path::new(&cfg_path).is_file() {
                    fig = fig.merge(Toml::file(&cfg_path));
                }

                fig
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
