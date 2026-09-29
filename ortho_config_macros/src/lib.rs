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
    //! Implementation modules for the `OrthoConfig` derive macro.

    pub(crate) mod build;
    pub(crate) mod crate_path;
    pub(crate) mod generate;
    pub(crate) mod load_impl;
    pub(crate) mod parse;
}
mod selected_subcommand_merge;
mod subcommand_docs;

use derive::build::cli_tokens::build_cli_struct_tokens;
use derive::build::{
    CollectionStrategies, DefaultStructInit, build_config_env_var, build_default_struct_fields,
    build_default_struct_init, build_env_provider, collect_collection_strategies,
    compute_config_env_var, compute_dotfile_name, default_app_name,
};
use derive::generate::declarative::generate_declarative_impl;
use derive::generate::docs::{DocsArgs, generate_docs_impl};
use derive::generate::ortho_impl::generate_trait_implementation;
use derive::load_impl::{
    DiscoveryTokens, LoadImplArgs, LoadImplIdents, LoadImplTokens, build_load_impl,
};
use derive::parse::{SerdeRenameAll, parse_input, serde_rename_all};

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

    let serde_rename_all = match serde_rename_all(&derive_input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let krate = derive::crate_path::resolve(struct_attrs.crate_path.as_ref());

    let component_args = MacroComponentArgs {
        ident: &ident,
        fields: &fields,
        struct_attrs: &struct_attrs,
        field_attrs: &field_attrs,
        serde_rename_all,
        krate: &krate,
    };

    let components = match build_macro_components(&component_args) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let core_tokens = generate_trait_implementation(&ident, &components, &krate);
    let declarative_impl = generate_declarative_impl(
        &ident,
        &components.collection_strategies,
        components.post_merge_hook,
        &krate,
    );
    let docs_impl = match generate_docs_impl(&DocsArgs {
        ident: &ident,
        fields: &fields,
        field_attrs: &field_attrs,
        struct_attrs: &struct_attrs,
        serde_rename_all,
        cli_fields: &components.cli_field_metadata,
        krate: &krate,
    }) {
        Ok(tokens) => tokens,
        Err(err) => return err.to_compile_error().into(),
    };
    let expanded = quote! {
        #core_tokens
        #declarative_impl
        #docs_impl
    };

    TokenStream::from(expanded)
}

/// Derive macro for the `ortho_config::SelectedSubcommandMerge` trait.
///
/// Apply this derive to a `clap::Subcommand` enum to generate a
/// `load_and_merge_selected` method that merges configuration defaults for the
/// selected variant.
///
/// Variants can opt into `ArgMatches`-aware merging (required for
/// `cli_default_as_absent` support) using `#[ortho_subcommand(with_matches)]`.
#[proc_macro_derive(SelectedSubcommandMerge, attributes(ortho_subcommand, ortho_config))]
pub fn derive_selected_subcommand_merge(input_tokens: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input_tokens as DeriveInput);
    match selected_subcommand_merge::derive_selected_subcommand_merge(derive_input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive macro for the `ortho_config::docs::OrthoConfigSubcommandDocs` trait.
///
/// Apply this derive to a `clap::Subcommand` enum whose variants each wrap a
/// single argument struct that implements `OrthoConfigDocs`.
#[proc_macro_derive(OrthoConfigSubcommandDocs, attributes(ortho_config))]
pub fn derive_subcommand_docs(input_tokens: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input_tokens as DeriveInput);
    match subcommand_docs::derive_subcommand_docs(derive_input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Metadata about a field used for `CliValueExtractor` generation.
#[derive(Clone)]
pub(crate) struct CliFieldInfo {
    /// The field name used by serde during serialization.
    ///
    /// This respects `#[serde(rename = "...")]` and `#[serde(rename_all = "...")]`
    /// so `CliValueExtractor` can reliably map values from `serde_json::to_value(self)`.
    pub serialized_key: String,
    /// The clap argument ID used with `ArgMatches::value_source()`.
    ///
    /// This is the `id = "..."` override from `#[arg(...)]`/`#[clap(...)]` when
    /// present, otherwise the field identifier (`snake_case`).
    pub arg_id: String,
    /// Whether this field has the `cli_default_as_absent` attribute.
    pub is_default_as_absent: bool,
}

/// Internal data generated during macro expansion.
///
/// These tokens and identifiers are kept together so each generator consumes
/// the same parsed field model and cannot silently diverge on configuration
/// precedence.
struct MacroComponents {
    /// Identifier of the synthetic struct that carries field defaults.
    defaults_ident: syn::Ident,
    /// Field declarations emitted into the synthetic defaults struct.
    default_struct_fields: Vec<proc_macro2::TokenStream>,
    /// Identifier of the synthetic `clap::Parser` struct.
    cli_ident: syn::Ident,
    /// Field declarations emitted into the synthetic CLI parser struct.
    cli_struct_fields: Vec<proc_macro2::TokenStream>,
    /// Implementation tokens that load and merge the configuration layers.
    load_impl: proc_macro2::TokenStream,
    /// Optional method returning the configured prefix for legacy discovery.
    prefix_fn: Option<proc_macro2::TokenStream>,
    /// Per-field collection merge rules shared by declarative merge generation.
    collection_strategies: CollectionStrategies,
    /// Field info for `CliValueExtractor` generation derived from the input
    /// configuration struct fields.
    ///
    /// This excludes any synthetic CLI-only fields generated by the macro (such
    /// as the optional `config_path` flag), because those fields are not present
    /// in the configuration struct and therefore cannot be extracted.
    cli_field_info: Vec<CliFieldInfo>,
    /// Field metadata for documentation IR generation.
    cli_field_metadata: Vec<derive::build::CliFieldMetadata>,
    /// Whether the struct has `#[ortho_config(post_merge_hook)]`.
    ///
    /// When true, the generated `merge_from_layers` invokes
    /// [`PostMergeHook::post_merge`] after declarative merging completes.
    post_merge_hook: bool,
}

/// References to tokens interpolated into generated `Load` implementations.
#[derive(Clone, Copy)]
struct LoadTokenRefs<'a> {
    /// Environment provider expression reused by generated load paths.
    env_provider: &'a proc_macro2::TokenStream,
    /// Constructor expression for the synthetic defaults value.
    default_struct_init: &'a DefaultStructInit,
    /// Generated expression naming the configuration environment variable.
    config_env_var: &'a proc_macro2::TokenStream,
    /// String literal used for the conventional dotfile candidate.
    dotfile_name: &'a syn::LitStr,
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
    /// Parsed settings shared with legacy and explicit discovery generation.
    struct_attrs: &'a derive::parse::StructAttrs,
    /// Optional values that override defaults derived from struct identity.
    discovery_tokens: Option<&'a DiscoveryTokens>,
    /// Whether the parser struct includes a user or generated config-path flag.
    has_config_path: bool,
}

/// Keeps generated load arguments alive alongside storage they borrow.
struct LoadImplResult<'a> {
    /// Parsed identifiers and token references consumed by the load generator.
    args: LoadImplArgs<'a>,
    /// Owns the legacy application name referenced from `args`.
    legacy_app_name_storage: String,
}

/// Inputs shared by the independent derive output generators.
struct MacroComponentArgs<'a> {
    /// Name of the user configuration struct being derived.
    ident: &'a syn::Ident,
    /// Original named fields, preserving source order for generated output.
    fields: &'a [syn::Field],
    /// Struct-level settings that control discovery and generated metadata.
    struct_attrs: &'a derive::parse::StructAttrs,
    /// Parsed per-field settings aligned with `fields` by index.
    field_attrs: &'a [derive::parse::FieldAttrs],
    /// Container-level Serde naming rule used by serialized field lookup.
    serde_rename_all: Option<SerdeRenameAll>,
    /// Resolved path to the downstream `ortho_config` crate.
    krate: &'a proc_macro2::TokenStream,
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
    krate: &'a proc_macro2::TokenStream,
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
        config_env_var,
        dotfile_name,
    } = token_refs;

    let tokens = LoadImplTokens {
        env_provider,
        default_struct_init,
        config_env_var,
        dotfile_name,
        legacy_app_name: legacy_app_name_storage.clone(),
        discovery: config.discovery_tokens,
        sources: None,
        krate,
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
fn build_macro_components(args: &MacroComponentArgs<'_>) -> syn::Result<MacroComponents> {
    let MacroComponentArgs {
        ident,
        fields,
        struct_attrs,
        field_attrs,
        serde_rename_all,
        krate,
    } = args;

    let defaults_ident = format_ident!("__{}Defaults", ident);
    let default_struct_fields = build_default_struct_fields(fields, field_attrs);
    let cli_ident = format_ident!("__{}Cli", ident);
    let cli_build_result =
        build_cli_struct_tokens(fields, field_attrs, struct_attrs, *serde_rename_all)?;
    let default_struct_init = build_default_struct_init(fields, field_attrs, krate);
    let env_provider = build_env_provider(struct_attrs, krate);
    let config_env_var = build_config_env_var(struct_attrs);
    let dotfile_name_string = compute_dotfile_name(struct_attrs);
    let dotfile_name = syn::LitStr::new(&dotfile_name_string, proc_macro2::Span::call_site());
    let collection_strategies = collect_collection_strategies(fields, field_attrs)?;
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
    } = build_load_impl_args(load_impl_idents, load_token_refs, load_impl_config, krate);
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
        cli_struct_fields: cli_build_result.fields,
        load_impl,
        prefix_fn,
        collection_strategies,
        cli_field_info: cli_build_result.field_info,
        cli_field_metadata: cli_build_result.metadata,
        post_merge_hook: struct_attrs.post_merge_hook,
    })
}

#[cfg(test)]
mod tests;
