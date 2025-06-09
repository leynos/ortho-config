//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file,
//! environment variables, and now command-line arguments via `clap`. CLI flag
//! names are automatically generated from `snake_case` field names using the
//! `kebab-case` convention.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};

mod derive {
    pub(crate) mod build;
    pub(crate) mod parse;
}

use derive::build::{
    LoadImplArgs, LoadImplIdents, LoadImplTokens, build_append_logic, build_cli_fields,
    build_config_env_var, build_default_struct_fields, build_default_struct_init,
    build_dotfile_name, build_env_provider, build_load_impl, build_override_struct,
    build_xdg_snippet, collect_append_fields,
};
use derive::parse::parse_input;

/// Derive macro for [`ortho_config::OrthoConfig`].
///
/// # Errors
///
/// Returns a compile-time error if invoked on a struct that contains unnamed fields.
#[proc_macro_derive(OrthoConfig, attributes(ortho_config))]
pub fn derive_ortho_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let (ident, fields, struct_attrs, field_attrs) = match parse_input(&input) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let cli_ident = format_ident!("__{}Cli", ident);
    let cli_mod = format_ident!("__{}CliMod", ident);
    let cli_pub_ident = format_ident!("{}Cli", ident);

    let cli_fields = build_cli_fields(&fields, &field_attrs);
    let defaults_ident = format_ident!("__{}Defaults", ident);
    let default_struct_fields = build_default_struct_fields(&fields);
    let default_struct_init = build_default_struct_init(&fields, &field_attrs);
    let env_provider = build_env_provider(&struct_attrs);
    let config_env_var = build_config_env_var(&struct_attrs);
    let dotfile_name = build_dotfile_name(&struct_attrs);
    let xdg_snippet = build_xdg_snippet(&struct_attrs);
    let append_fields = collect_append_fields(&fields, &field_attrs);
    let (override_struct_ts, override_init_ts) = build_override_struct(&ident, &append_fields);
    let append_logic = build_append_logic(&append_fields);
    let load_impl = build_load_impl(&LoadImplArgs {
        idents: LoadImplIdents {
            ident: &ident,
            cli_mod: &cli_mod,
            cli_ident: &cli_ident,
            defaults_ident: &defaults_ident,
        },
        tokens: LoadImplTokens {
            env_provider: &env_provider,
            default_struct_init: &default_struct_init,
            override_init_ts: &override_init_ts,
            append_logic: &append_logic,
            config_env_var: &config_env_var,
            dotfile_name: &dotfile_name,
            xdg_snippet: &xdg_snippet,
        },
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

        #[derive(serde::Serialize)]
        struct #defaults_ident {
            #( #default_struct_fields, )*
        }

        #override_struct_ts

        pub use #cli_mod::#cli_ident as #cli_pub_ident;

        #load_impl

        impl ortho_config::OrthoConfig for #ident {
            fn load() -> Result<Self, ortho_config::OrthoError> {
                Self::load_from_iter(::std::env::args_os())
            }
        }
    };

    TokenStream::from(expanded)
}
