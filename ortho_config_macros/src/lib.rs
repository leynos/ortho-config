//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file,
//! environment variables, and now command-line arguments via `clap`. CLI flag
//! names are automatically generated from `snake_case` field names by replacing
//! underscores with hyphens. Generated long flags therefore never include
//! underscores; supply `cli_long` to specify them.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};

mod derive {
    pub(crate) mod build;
    pub(crate) mod load_impl;
    pub(crate) mod parse;
}

use derive::build::{
    build_append_logic, build_cli_struct_fields, build_config_env_var, build_config_flag_field,
    build_default_struct_fields, build_default_struct_init, build_env_provider,
    build_override_struct, collect_append_fields, compute_config_env_var, compute_dotfile_name,
    default_app_name,
};
use derive::load_impl::{
    DiscoveryTokens, LoadImplArgs, LoadImplIdents, LoadImplTokens, build_load_impl,
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

    let components = match build_macro_components(&ident, &fields, &struct_attrs, &field_attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let expanded = generate_trait_implementation(&ident, &components);

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

#[derive(Clone, Copy)]
struct LoadTokenRefs<'a> {
    env_provider: &'a proc_macro2::TokenStream,
    default_struct_init: &'a [proc_macro2::TokenStream],
    override_init_ts: &'a proc_macro2::TokenStream,
    append_logic: &'a proc_macro2::TokenStream,
    config_env_var: &'a proc_macro2::TokenStream,
    dotfile_name: &'a syn::LitStr,
}

/// Build CLI struct field tokens, conditionally adding a generated
/// `config_path` field.
///
/// If no user-defined `config_path` field exists, generates a config flag field
/// based on discovery attributes from `struct_attrs`.
///
/// # Arguments
///
/// - `fields`: Struct fields used to generate CLI tokens.
/// - `field_attrs`: Per-field attributes controlling CLI generation.
/// - `struct_attrs`: Struct-level attributes including discovery settings.
///
/// # Errors
///
/// Returns an error if CLI flag collisions are detected.
fn build_cli_struct_tokens(
    fields: &[syn::Field],
    field_attrs: &[derive::parse::FieldAttrs],
    struct_attrs: &derive::parse::StructAttrs,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let has_user_config_path = fields.iter().any(|field| {
        field
            .ident
            .as_ref()
            .is_some_and(|ident| ident == "config_path")
    });
    let mut cli_struct = build_cli_struct_fields(fields, field_attrs)?;
    if !has_user_config_path {
        let config_field = build_config_flag_field(
            struct_attrs,
            &cli_struct.used_shorts,
            &cli_struct.used_longs,
        )?;
        cli_struct.fields.push(config_field);
    }
    Ok(cli_struct.fields)
}

/// Build discovery tokens from struct-level `discovery(...)` attributes.
///
/// Populates discovery settings with defaults derived from
/// `struct_attrs.prefix` and `ident` when explicit values are not provided.
/// Returns `None` if no discovery attribute is present.
///
/// # Arguments
///
/// - `struct_attrs`: Struct-level attributes that may include discovery
///   configuration.
/// - `ident`: The struct identifier used to derive default app names.
fn build_discovery_tokens(
    struct_attrs: &derive::parse::StructAttrs,
    ident: &syn::Ident,
) -> Option<DiscoveryTokens> {
    let default_app_name_value = default_app_name(struct_attrs, ident);
    struct_attrs
        .discovery
        .as_ref()
        .map(|attrs| DiscoveryTokens {
            app_name: attrs.app_name.clone().unwrap_or(default_app_name_value),
            env_var: attrs
                .env_var
                .clone()
                .unwrap_or_else(|| compute_config_env_var(struct_attrs)),
            config_file_name: attrs.config_file_name.clone(),
            dotfile_name: attrs.dotfile_name.clone(),
            project_file_name: attrs.project_file_name.clone(),
        })
}

#[derive(Clone, Copy)]
struct LoadImplConfig<'a> {
    struct_attrs: &'a derive::parse::StructAttrs,
    discovery_tokens: Option<&'a DiscoveryTokens>,
    has_config_path: bool,
}

/// Construct `LoadImplArgs` by combining identifiers, token references, and
/// configuration.
///
/// Computes the legacy app name from the optional prefix so the legacy
/// discovery flow can reuse the builder without mutating state in the caller.
///
/// # Arguments
///
/// - `idents`: CLI, config, and defaults struct identifiers.
/// - `token_refs`: References to token streams used in the load
///   implementation.
/// - `config`: Configuration grouping discovery tokens and config-path
///   presence.
fn build_load_impl_args<'a>(
    idents: LoadImplIdents<'a>,
    token_refs: LoadTokenRefs<'a>,
    config: LoadImplConfig<'a>,
) -> LoadImplArgs<'a> {
    let legacy_app_name = config
        .struct_attrs
        .prefix
        .as_ref()
        .map(|prefix| prefix.trim_end_matches('_').to_ascii_lowercase())
        .unwrap_or_default();
    let LoadTokenRefs {
        env_provider,
        default_struct_init,
        override_init_ts,
        append_logic,
        config_env_var,
        dotfile_name,
    } = token_refs;

    LoadImplArgs {
        idents,
        tokens: LoadImplTokens {
            env_provider,
            default_struct_init,
            override_init_ts,
            append_logic,
            config_env_var,
            dotfile_name,
            legacy_app_name,
            discovery: config.discovery_tokens,
        },
        has_config_path: config.has_config_path,
    }
}

/// Build all components required for macro output.
fn build_macro_components(
    ident: &syn::Ident,
    fields: &[syn::Field],
    struct_attrs: &derive::parse::StructAttrs,
    field_attrs: &[derive::parse::FieldAttrs],
) -> syn::Result<MacroComponents> {
    let defaults_ident = format_ident!("__{}Defaults", ident);
    let default_struct_fields = build_default_struct_fields(fields);
    let cli_ident = format_ident!("__{}Cli", ident);
    let cli_struct_fields = build_cli_struct_tokens(fields, field_attrs, struct_attrs)?;
    let default_struct_init = build_default_struct_init(fields, field_attrs);
    let env_provider = build_env_provider(struct_attrs);
    let config_env_var = build_config_env_var(struct_attrs);
    let dotfile_name_string = compute_dotfile_name(struct_attrs);
    let dotfile_name = syn::LitStr::new(&dotfile_name_string, proc_macro2::Span::call_site());
    let append_fields = collect_append_fields(fields, field_attrs);
    let (override_struct_ts, override_init_ts) = build_override_struct(ident, &append_fields);
    let append_logic = build_append_logic(&append_fields);
    let has_config_path = true;
    let discovery_tokens = build_discovery_tokens(struct_attrs, ident);
    let load_impl_idents = LoadImplIdents {
        cli_ident: &cli_ident,
        config_ident: ident,
        defaults_ident: &defaults_ident,
    };
    let load_token_refs = LoadTokenRefs {
        env_provider: &env_provider,
        default_struct_init: &default_struct_init,
        override_init_ts: &override_init_ts,
        append_logic: &append_logic,
        config_env_var: &config_env_var,
        dotfile_name: &dotfile_name,
    };
    let load_impl_config = LoadImplConfig {
        struct_attrs,
        discovery_tokens: discovery_tokens.as_ref(),
        has_config_path,
    };
    let load_impl_args = build_load_impl_args(load_impl_idents, load_token_refs, load_impl_config);
    let load_impl = build_load_impl(&load_impl_args);
    let prefix_fn = struct_attrs.prefix.as_ref().map(|prefix| {
        quote! {
            fn prefix() -> &'static str {
                #prefix
            }
        }
    });

    Ok(MacroComponents {
        defaults_ident,
        default_struct_fields,
        cli_ident,
        cli_struct_fields,
        override_struct_ts,
        load_impl,
        prefix_fn,
    })
}

/// Generate the hidden `clap::Parser` struct.
fn generate_cli_struct(components: &MacroComponents) -> proc_macro2::TokenStream {
    let MacroComponents {
        cli_ident,
        cli_struct_fields,
        ..
    } = components;
    quote! {
        #[derive(clap::Parser, serde::Serialize, Default)]
        struct #cli_ident {
            #( #cli_struct_fields, )*
        }
    }
}

/// Generate the struct used to store default values.
fn generate_defaults_struct(components: &MacroComponents) -> proc_macro2::TokenStream {
    let MacroComponents {
        defaults_ident,
        default_struct_fields,
        ..
    } = components;
    quote! {
        #[derive(serde::Serialize)]
        struct #defaults_ident {
            #( #default_struct_fields, )*
        }
    }
}

/// Generate the `OrthoConfig` trait implementation.
fn generate_ortho_impl(
    config_ident: &syn::Ident,
    components: &MacroComponents,
) -> proc_macro2::TokenStream {
    let MacroComponents {
        cli_ident,
        override_struct_ts,
        load_impl,
        prefix_fn,
        ..
    } = components;
    let prefix_fn = prefix_fn.clone().unwrap_or_else(|| quote! {});
    quote! {
        #override_struct_ts

        #load_impl

        impl ortho_config::OrthoConfig for #config_ident {
            fn load_from_iter<I, T>(iter: I) -> ortho_config::OrthoResult<Self>
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

fn generate_trait_implementation(
    config_ident: &syn::Ident,
    components: &MacroComponents,
) -> proc_macro2::TokenStream {
    let cli_struct = generate_cli_struct(components);
    let defaults_struct = generate_defaults_struct(components);
    let ortho_impl = generate_ortho_impl(config_ident, components);
    quote! {
        #cli_struct
        #defaults_struct
        #ortho_impl
    }
}
