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
    pub(crate) mod load_impl;
    pub(crate) mod parse;
}

use derive::build::{
    build_append_logic, build_cli_struct_fields, build_config_env_var, build_default_struct_fields,
    build_default_struct_init, build_dotfile_name, build_env_provider, build_override_struct,
    build_xdg_snippet, collect_append_fields,
};
use derive::load_impl::{LoadImplArgs, LoadImplIdents, LoadImplTokens, build_load_impl};
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

    let components = build_macro_components(&ident, &fields, &struct_attrs, &field_attrs);
    let defaults_struct = generate_defaults_struct(
        &components.defaults_ident,
        &components.default_struct_fields,
    );
    let cli_struct = generate_cli_struct(&components.cli_ident, &components.cli_struct_fields);
    let trait_impl = generate_trait_implementation(
        &ident,
        &components.cli_ident,
        &components.load_impl,
        components.prefix_fn,
    );
    let override_struct_ts = components.override_struct_ts;

    let expanded = quote! {
        #cli_struct
        #defaults_struct
        #override_struct_ts
        #trait_impl
    };

    TokenStream::from(expanded)
}

/// Internal data generated during macro expansion.
struct MacroComponents {
    defaults_ident: syn::Ident,
    default_struct_fields: Vec<proc_macro2::TokenStream>,
    cli_ident: syn::Ident,
    cli_struct_fields: Vec<proc_macro2::TokenStream>,
    override_struct_ts: proc_macro2::TokenStream,
    load_impl: proc_macro2::TokenStream,
    prefix_fn: Option<proc_macro2::TokenStream>,
}

/// Build all components required for macro output.
fn build_macro_components(
    ident: &syn::Ident,
    fields: &[syn::Field],
    struct_attrs: &derive::parse::StructAttrs,
    field_attrs: &[derive::parse::FieldAttrs],
) -> MacroComponents {
    let defaults_ident = format_ident!("__{}Defaults", ident);
    let default_struct_fields = build_default_struct_fields(fields);
    let cli_ident = format_ident!("__{}Cli", ident);
    let cli_struct_fields = build_cli_struct_fields(fields, field_attrs);
    let default_struct_init = build_default_struct_init(fields, field_attrs);
    let env_provider = build_env_provider(struct_attrs);
    let config_env_var = build_config_env_var(struct_attrs);
    let dotfile_name = build_dotfile_name(struct_attrs);
    let xdg_snippet = build_xdg_snippet(struct_attrs);
    let append_fields = collect_append_fields(fields, field_attrs);
    let (override_struct_ts, override_init_ts) = build_override_struct(ident, &append_fields);
    let append_logic = build_append_logic(&append_fields);
    let has_config_path = fields
        .iter()
        .any(|f| f.ident.as_ref().is_some_and(|id| id == "config_path"));
    let load_impl = build_load_impl(&LoadImplArgs {
        idents: LoadImplIdents {
            cli_ident: &cli_ident,
            config_ident: ident,
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
        has_config_path,
    });
    let prefix_fn = struct_attrs.prefix.as_ref().map(|prefix| {
        quote! {
            fn prefix() -> &'static str {
                #prefix
            }
        }
    });

    MacroComponents {
        defaults_ident,
        default_struct_fields,
        cli_ident,
        cli_struct_fields,
        override_struct_ts,
        load_impl,
        prefix_fn,
    }
}

/// Generate the hidden defaults struct for the macro output.
fn generate_defaults_struct(
    ident: &syn::Ident,
    fields: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        #[derive(serde::Serialize)]
        struct #ident {
            #( #fields, )*
        }
    }
}

/// Generate the `OrthoConfig` trait implementation.
fn generate_cli_struct(
    ident: &syn::Ident,
    fields: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        #[derive(clap::Parser, serde::Serialize)]
        struct #ident {
            #( #fields, )*
        }
    }
}

fn generate_trait_implementation(
    config_ident: &syn::Ident,
    cli_ident: &syn::Ident,
    load_impl: &proc_macro2::TokenStream,
    prefix_fn: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let prefix_fn = prefix_fn.unwrap_or_else(|| quote! {});
    quote! {
        #load_impl

        impl ortho_config::OrthoConfig for #config_ident {
            fn load_from_iter<I, T>(iter: I) -> Result<Self, ortho_config::OrthoError>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #cli_ident::load_from_iter(iter)
            }

            #prefix_fn
        }

        const _: () = {
            fn _assert_deser<T: serde::de::DeserializeOwned>() {}
            let _ = _assert_deser::<#config_ident>;
        };
    }
}
