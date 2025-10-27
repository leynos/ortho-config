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
    pub(crate) mod generate;
    pub(crate) mod load_impl;
    pub(crate) mod parse;
}

use derive::build::{
    CollectionStrategies, build_cli_struct_fields, build_collection_logic, build_config_env_var,
    build_config_flag_field, build_default_struct_fields, build_default_struct_init,
    build_env_provider, build_override_struct, collect_collection_strategies,
    compute_config_env_var, compute_dotfile_name, default_app_name,
};
use derive::generate::declarative::generate_declarative_impl;
use derive::generate::ortho_impl::generate_trait_implementation;
use derive::load_impl::{
    DiscoveryTokens, LoadImplArgs, LoadImplIdents, LoadImplTokens, build_load_impl,
};
use derive::parse::parse_input;

/// Derive macro for the
/// [`OrthoConfig`](https://docs.rs/ortho_config/latest/ortho_config/trait.OrthoConfig.html)
/// trait.
///
/// # Errors
///
/// Returns a compile-time error if invoked on a struct that contains unnamed fields.
#[proc_macro_derive(OrthoConfig, attributes(ortho_config))]
pub fn derive_ortho_config(input_tokens: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input_tokens as DeriveInput);
    let (ident, fields, struct_attrs, field_attrs) = match parse_input(&derive_input) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let components = match build_macro_components(&ident, &fields, &struct_attrs, &field_attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let core_tokens = generate_trait_implementation(&ident, &components);
    let declarative_impl = generate_declarative_impl(&ident, &components.collection_strategies);
    let expanded = quote! {
        #core_tokens
        #declarative_impl
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
    collection_strategies: CollectionStrategies,
}

#[derive(Clone, Copy)]
struct LoadTokenRefs<'a> {
    env_provider: &'a proc_macro2::TokenStream,
    default_struct_init: &'a [proc_macro2::TokenStream],
    override_init_ts: &'a proc_macro2::TokenStream,
    collection_pre_merge: &'a proc_macro2::TokenStream,
    collection_post_extract: &'a proc_macro2::TokenStream,
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
    let mut cli_struct = build_cli_struct_fields(fields, field_attrs)?;
    if !cli_struct.field_names.contains("config_path") {
        let config_field = build_config_flag_field(
            struct_attrs,
            &cli_struct.used_shorts,
            &cli_struct.used_longs,
            &cli_struct.field_names,
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

/// Aggregated configuration for constructing load-implementation tokens.
///
/// Bundles the struct-level attributes, optional discovery settings, and
/// whether a `config_path` field was supplied so helper functions can reason
/// about discovery without a long parameter list.
#[derive(Clone, Copy)]
struct LoadImplConfig<'a> {
    struct_attrs: &'a derive::parse::StructAttrs,
    discovery_tokens: Option<&'a DiscoveryTokens>,
    has_config_path: bool,
}

struct LoadImplResult<'a> {
    args: LoadImplArgs<'a>,
    legacy_app_name_storage: String,
}

/// Construct `LoadImplArgs` from identifiers, token references, and discovery
/// configuration.
///
/// Computes the legacy app name from the optional prefix and stores it inside
/// the returned [`LoadImplTokens`], allowing the legacy discovery path to reuse
/// the shared builder flow. Aggregates the provided [`LoadImplConfig`] so
/// callers do not have to pass multiple loosely related arguments.
///
/// # Arguments
///
/// - `idents`: CLI, config, and defaults struct identifiers.
/// - `token_refs`: References to token streams used in the load
///   implementation.
/// - `config`: Configuration grouping struct attributes, discovery tokens, and
///   config-path presence.
fn build_load_impl_args<'a>(
    idents: LoadImplIdents<'a>,
    token_refs: LoadTokenRefs<'a>,
    config: LoadImplConfig<'a>,
) -> LoadImplResult<'a> {
    let legacy_app_name_storage = config
        .struct_attrs
        .prefix
        .as_ref()
        .map(|prefix| prefix.trim_end_matches('_').to_ascii_lowercase())
        .unwrap_or_default();
    let LoadTokenRefs {
        env_provider,
        default_struct_init,
        override_init_ts,
        collection_pre_merge,
        collection_post_extract,
        config_env_var,
        dotfile_name,
    } = token_refs;

    let tokens = LoadImplTokens {
        env_provider,
        default_struct_init,
        override_init_ts,
        collection_pre_merge,
        collection_post_extract,
        config_env_var,
        dotfile_name,
        legacy_app_name: legacy_app_name_storage.clone(),
        discovery: config.discovery_tokens,
    };
    let args = LoadImplArgs {
        idents,
        tokens,
        has_config_path: config.has_config_path,
    };
    LoadImplResult {
        args,
        legacy_app_name_storage,
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
    let collection_strategies = collect_collection_strategies(fields, field_attrs)?;
    let (override_struct_ts, override_init_ts) =
        build_override_struct(ident, &collection_strategies);
    let cli_binding = if cli_struct_fields.is_empty() {
        quote!(std::option::Option::<&()>::None)
    } else {
        quote!(cli.as_ref())
    };
    let collection_logic_tokens = build_collection_logic(&collection_strategies, &cli_binding);
    // has_config_path is always true: either user-defined or generated by
    // build_cli_struct_tokens.
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
        collection_pre_merge: &collection_logic_tokens.pre_merge,
        collection_post_extract: &collection_logic_tokens.post_extract,
        config_env_var: &config_env_var,
        dotfile_name: &dotfile_name,
    };
    let load_impl_config = LoadImplConfig {
        struct_attrs,
        discovery_tokens: discovery_tokens.as_ref(),
        has_config_path,
    };
    let LoadImplResult {
        args: load_impl_args,
        legacy_app_name_storage: _legacy_app_name_storage,
    } = build_load_impl_args(load_impl_idents, load_token_refs, load_impl_config);
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
        collection_strategies,
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the procedural macro token generators.

    use super::MacroComponents;
    use crate::derive::build::CollectionStrategies;
    use crate::derive::generate::structs::{
        generate_cli_struct, generate_defaults_struct, generate_struct,
    };
    use anyhow::{Context, Result, ensure};
    use proc_macro2::TokenStream as TokenStream2;
    use quote::quote;
    use rstest::rstest;
    use syn::parse_str;

    fn build_components(
        default_struct_fields: Vec<TokenStream2>,
        cli_struct_fields: Vec<TokenStream2>,
    ) -> Result<MacroComponents> {
        Ok(MacroComponents {
            defaults_ident: parse_str("DefaultsStruct").context("defaults ident")?,
            default_struct_fields,
            cli_ident: parse_str("CliStruct").context("cli ident")?,
            cli_struct_fields,
            override_struct_ts: quote! {},
            load_impl: quote! {},
            prefix_fn: None,
            collection_strategies: CollectionStrategies::default(),
        })
    }

    #[rstest]
    fn generate_struct_handles_empty_fields() -> Result<()> {
        let ident = parse_str("Empty").context("parse Empty ident")?;
        let attrs = quote! { #[derive(Default)] };
        let tokens = generate_struct(&ident, &[], &attrs);
        let expected = quote! {
            #[derive(Default)]
            struct Empty {}
        };
        ensure!(
            tokens.to_string() == expected.to_string(),
            "generated tokens differ: {tokens} != {expected}"
        );
        Ok(())
    }

    #[rstest]
    fn generate_struct_renders_fields_with_commas() -> Result<()> {
        let ident = parse_str("WithFields").context("parse WithFields ident")?;
        let fields = vec![quote! { pub value: u32 }, quote! { pub other: String }];
        let attrs = quote! { #[derive(Default)] };
        let tokens = generate_struct(&ident, &fields, &attrs);
        let expected = quote! {
            #[derive(Default)]
            struct WithFields {
                pub value: u32,
                pub other: String,
            }
        };
        ensure!(
            tokens.to_string() == expected.to_string(),
            "generated tokens differ: {tokens} != {expected}"
        );
        Ok(())
    }

    #[rstest]
    fn generate_cli_struct_emits_expected_tokens() -> Result<()> {
        let components = build_components(
            vec![quote! { pub value: u32 }],
            vec![quote! { #[clap(long)] pub value: Option<u32> }],
        )?;
        let tokens = generate_cli_struct(&components);
        let expected = quote! {
            #[derive(clap::Parser, serde::Serialize, Default)]
            struct CliStruct {
                #[clap(long)]
                pub value: Option<u32>,
            }
        };
        ensure!(
            tokens.to_string() == expected.to_string(),
            "generated CLI struct differs: {tokens} != {expected}"
        );
        Ok(())
    }

    #[rstest]
    fn generate_defaults_struct_supports_empty_fields() -> Result<()> {
        let components = build_components(
            Vec::new(),
            vec![quote! { #[clap(long)] pub value: Option<u32> }],
        )?;
        let tokens = generate_defaults_struct(&components);
        let expected = quote! {
            #[derive(serde::Serialize)]
            struct DefaultsStruct {}
        };
        ensure!(
            tokens.to_string() == expected.to_string(),
            "generated defaults struct differs: {tokens} != {expected}"
        );
        Ok(())
    }
}
