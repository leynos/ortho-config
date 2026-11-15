//! Helpers for generating the `load_and_merge` implementation.
//!
//! This module assembles legacy composition and delegates discovery, profile
//! composition, and public entry-point assembly to focused child modules.

use quote::quote;
use syn::Ident;

use crate::derive::build::DefaultStructInit;

pub(crate) mod cli;
use cli::{build_cli_layer_tokens, build_cli_parse_tokens};

mod discovery;
use self::discovery::build_file_discovery;
mod source;
pub(crate) use source::LoadSourceTokens;

/// Identifiers used when generating the load implementation.
#[expect(
    clippy::struct_field_names,
    reason = "Field names mirror their purpose for clarity"
)]
pub(crate) struct LoadImplIdents<'a> {
    /// Name of the generated CLI struct that owns argument parsing.
    pub cli_ident: &'a Ident,
    /// Name of the user-facing config struct constructed from merged layers.
    pub config_ident: &'a Ident,
    /// Internal type used to materialize field defaults before merging.
    pub defaults_ident: &'a Ident,
}
/// Token collections used by the load implementation helpers.
pub(crate) struct LoadImplTokens<'a> {
    /// Figment provider tokens used to read the configuration environment.
    pub env_provider: &'a proc_macro2::TokenStream,
    /// Resolved field initializers and fallible default conversions.
    pub default_struct_init: &'a DefaultStructInit,
    /// Tokenized environment-variable name for legacy config-file lookup.
    pub config_env_var: &'a proc_macro2::TokenStream,
    /// Legacy dotfile literal supplied when discovery settings are absent.
    pub dotfile_name: &'a syn::LitStr,
    /// Application name used to derive legacy discovery locations.
    pub legacy_app_name: String,
    /// Explicit discovery settings, when the derive input supplied them.
    pub discovery: Option<&'a DiscoveryTokens>,
    /// Runtime source tokens for the source-aware generated entry points.
    pub sources: Option<LoadSourceTokens<'a>>,
    /// Resolved crate path for generated code references.
    pub krate: &'a proc_macro2::TokenStream,
}

/// Parsed discovery settings shared by legacy and policy token emitters.
pub(crate) struct DiscoveryTokens {
    /// Application identity used to construct the discovery builder.
    pub app_name: String,
    /// Materialized default config-path variable used by the legacy path.
    pub env_var: String,
    /// Optional conventional config filename override.
    pub config_file_name: Option<String>,
    /// Optional hidden-file name override.
    pub dotfile_name: Option<String>,
    /// Optional project-local filename override.
    pub project_file_name: Option<String>,
    /// Selects the policy resolver instead of the legacy discovery walk.
    pub policy_enabled: bool,
    /// Ordered selector aliases; the first present variable takes precedence.
    pub env_vars: Vec<String>,
    /// Optional required/optional mode for an explicitly selected path.
    pub explicit_mode: Option<String>,
    /// Optional precedence mode used by automatic discovery.
    pub automatic_mode: Option<String>,
    /// Declared scope precedence, preserved in emitted builder calls.
    pub scope_order: Vec<String>,
    /// Names the CLI field whose parsed value becomes the project root.
    ///
    /// The field's declared type is validated by the parser but not recorded
    /// here: the generated CLI struct wraps *every* field in `Option`, so the
    /// emitter reads an `Option` whichever type was written.
    pub project_root_from: Option<String>,
}

/// Convenience wrapper for passing identifiers and tokens together.
pub(crate) struct LoadImplArgs<'a> {
    /// Generated type identifiers referenced by the emitted implementation.
    pub idents: LoadImplIdents<'a>,
    /// Providers, defaults, discovery settings, and crate path for expansion.
    pub tokens: LoadImplTokens<'a>,
    /// Whether the generated CLI struct contains the required path selector.
    pub has_config_path: bool,
    /// Whether the struct opts into profile support (`#[ortho_config(profiles)]`).
    pub profiles: bool,
    /// The selector environment variable name (for example `APP_PROFILE`).
    pub profile_env_var: String,
    /// Clap argument IDs of the config fields (excluding the generated
    /// `profile` and `config_path` fields), used for value-source gating of
    /// the CLI layer push.
    pub cli_arg_ids: Vec<String>,
    /// `(arg_id, serialized_key)` for every bool config field.
    ///
    /// Ordinary boolean absence is `None` and its serialized `null` is stripped.
    /// Actual clap defaults can survive sanitization, however, and must not
    /// mask the lower-precedence profile layer. Pruning checks `value_source`
    /// through the argument ID, then removes the matching serialized JSON key;
    /// an explicit command-line `false` is retained.
    pub cli_bool_fields: Vec<(String, String)>,
}

/// Build the environment provider setup.
///
/// Environment variables sit just below CLI arguments in the precedence order,
/// using a prefix if one was supplied in the macro input. See the "Primary data
/// flow" and "The `OrthoConfig` Trait" sections in the design document.
pub(crate) fn build_env_section(tokens: &LoadImplTokens<'_>) -> proc_macro2::TokenStream {
    let env_provider = tokens.env_provider;
    let merge_source = tokens.sources.as_ref().map(|source_tokens| {
        let injected_source = source_tokens.merge;
        quote! { .with_source(#injected_source) }
    });
    quote! {
        let env_provider = {
            #env_provider
                #merge_source
        };
    }
}

/// Emit default-layer construction and retain conversion failures in `errors`.
///
/// # Example
///
/// If the resolved defaults serialize as `{ retries: 3 }`, the emitted setup
/// pushes that value as the lowest-priority layer; a sanitization failure is
/// added to `errors` instead.
fn build_legacy_defaults(
    krate: &proc_macro2::TokenStream,
    defaults_ident: &Ident,
    default_struct_init: &DefaultStructInit,
) -> proc_macro2::TokenStream {
    let default_resolutions = &default_struct_init.resolutions;
    let default_fields = &default_struct_init.fields;
    quote! {
        let mut composer = #krate::MergeComposer::with_capacity(4);
        #(#default_resolutions)*
        let defaults = #defaults_ident { #( #default_fields, )* };
        match #krate::sanitize_value(&defaults) {
            Ok(value) => composer.push_defaults(value),
            Err(err) => errors.push(err),
        }
    }
}

/// Emit the legacy file-layer push, preserving discovery order for merging.
///
/// # Example
///
/// If discovery returns a base file followed by a local override, the emitted
/// loop pushes both as separate layers in that order for later composition.
fn build_legacy_file_layers(file_discovery: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! {
        // The legacy path merges every file layer directly and never reports an
        // unknown profile, so the discovery-failure flag has no consumer here.
        let (file_layers, _discovery_failed) = #file_discovery;
        for layer in file_layers {
            composer.push_layer(layer);
        }
    }
}

/// Emit environment extraction and append conversion failures to `errors`.
///
/// # Example
///
/// With `APP_RETRIES=5`, the emitted provider extracts the environment layer
/// and pushes it after file layers; an extraction failure is collected in
/// `errors`.
fn build_legacy_environment_layer(
    krate: &proc_macro2::TokenStream,
    env_section: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #env_section
        match Figment::from(env_provider.clone())
            .extract::<#krate::serde_json::Value>()
            .into_ortho_merge()
        {
            Ok(value) => composer.push_environment(value),
            Err(err) => errors.push(err),
        }
    }
}

/// Generate the composition body shared by iterator-based loading methods.
///
/// The emitted order is defaults, discovered file layers, environment, then
/// CLI, so later sources retain their documented higher precedence. Errors
/// from each conversion are accumulated with the composition result.
fn build_compose_layers_impl(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplArgs {
        idents,
        tokens,
        has_config_path,
        profiles,
        ..
    } = args;
    let defaults_ident = idents.defaults_ident;
    let default_struct_init = tokens.default_struct_init;
    let cli_default_as_absent_fields = &default_struct_init.cli_default_as_absent_fields;
    let krate = tokens.krate;
    let file_discovery = build_file_discovery(tokens, *has_config_path);
    let env_section = build_env_section(tokens);

    if *profiles {
        load_impl_profiles::build_profile_compose_layers_impl(args, &file_discovery, &env_section)
    } else {
        let parse_setup = build_cli_parse_tokens();
        let defaults = build_legacy_defaults(krate, defaults_ident, default_struct_init);
        let file_layers = build_legacy_file_layers(&file_discovery);
        let environment_layer = build_legacy_environment_layer(krate, &env_section);
        let cli_push = build_cli_layer_tokens(krate, cli_default_as_absent_fields);
        quote! {
            use clap::{CommandFactory as _, FromArgMatches as _, Parser as _};
            // Keep this path anchored under the resolved crate so derive users
            // do not need a direct `figment` dependency for macro-generated code.
            use #krate::figment::Figment;
            use #krate::OrthoMergeExt as _;

            let mut errors: Vec<std::sync::Arc<#krate::OrthoError>> = Vec::new();
            #parse_setup
            #defaults
            #file_layers
            #environment_layer

            #cli_push

            #krate::declarative::LayerComposition::new(composer.layers(), errors)
        }
    }
}

#[path = "../load_impl_entry.rs"]
mod load_impl_entry;
#[path = "../load_impl_profiles.rs"]
mod load_impl_profiles;
pub(crate) use load_impl_entry::build_load_impl;
