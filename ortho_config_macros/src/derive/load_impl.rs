//! Helpers for generating the `load_and_merge` implementation.
//!
//! These functions assemble the configuration loading logic by piecing together
//! CLI parsing, file discovery, environment provider setup, and final merging
//! according to the order described in the design document. The profile-enabled
//! compose body lives in the sibling [`load_impl_profiles`] module and the
//! entry-point assembly in [`load_impl_entry`].

#[path = "load_impl_entry.rs"]
mod load_impl_entry;
#[path = "load_impl_profiles.rs"]
mod load_impl_profiles;

pub(crate) use load_impl_entry::build_load_impl;

use quote::quote;
use syn::Ident;

/// Identifiers used when generating the load implementation.
#[expect(
    clippy::struct_field_names,
    reason = "Field names mirror their purpose for clarity"
)]
pub(crate) struct LoadImplIdents<'a> {
    pub cli_ident: &'a Ident,
    pub config_ident: &'a Ident,
    pub defaults_ident: &'a Ident,
}

/// Token collections used by the load implementation helpers.
pub(crate) struct LoadImplTokens<'a> {
    pub env_provider: &'a proc_macro2::TokenStream,
    pub default_struct_init: &'a [proc_macro2::TokenStream],
    pub config_env_var: &'a proc_macro2::TokenStream,
    pub dotfile_name: &'a syn::LitStr,
    pub legacy_app_name: String,
    pub discovery: Option<&'a DiscoveryTokens>,
    /// Resolved crate path for generated code references.
    pub krate: &'a proc_macro2::TokenStream,
}

pub(crate) struct DiscoveryTokens {
    pub app_name: String,
    pub env_var: String,
    pub config_file_name: Option<String>,
    pub dotfile_name: Option<String>,
    pub project_file_name: Option<String>,
}

/// Convenience wrapper for passing identifiers and tokens together.
pub(crate) struct LoadImplArgs<'a> {
    pub idents: LoadImplIdents<'a>,
    pub tokens: LoadImplTokens<'a>,
    pub has_config_path: bool,
    /// Whether the struct opts into profile support (`#[ortho_config(profiles)]`).
    pub profiles: bool,
    /// The selector environment variable name (for example `APP_PROFILE`).
    pub profile_env_var: String,
    /// Clap argument IDs of the config fields (excluding the generated
    /// `profile` and `config_path` fields), used for value-source gating of
    /// the CLI layer push.
    pub cli_arg_ids: Vec<String>,
}

/// CLI parsing is performed outside the generated method.
///
/// Generate the file discovery logic section.
///
/// Configuration files are searched in multiple locations as described in the
/// "Configuration File Discovery" section of the design document. This mirrors
/// standard XDG behaviour on Unix-like systems and uses `directories` on Windows.
fn to_lit_str(value: Option<&String>) -> Option<syn::LitStr> {
    value.map(|contents| syn::LitStr::new(contents, proc_macro2::Span::call_site()))
}

fn build_optional_stmt(
    lit: Option<syn::LitStr>,
    method_name: &str,
) -> Option<proc_macro2::TokenStream> {
    lit.map(|lit_str| {
        let method_ident = syn::Ident::new(method_name, proc_macro2::Span::call_site());
        quote! { builder = builder.#method_ident(#lit_str); }
    })
}

fn build_cli_chain_tokens(has_config_path: bool) -> proc_macro2::TokenStream {
    if has_config_path {
        quote! {
            if let Some(ref cli) = cli {
                if let Some(ref path) = cli.config_path {
                    builder = builder.add_required_path(path.clone());
                }
            }
        }
    } else {
        quote! {}
    }
}

/// Generate discovery loading tokens with partitioned error handling.
///
/// Creates a code block that builds a `ConfigDiscovery`, loads the first
/// available configuration file using partitioned error reporting, and
/// conditionally appends optional discovery errors only when no file loads.
/// Required-path errors are always appended to the main error collection to
/// preserve the builder API's guarantees.
///
/// This uses `compose_layers()` to preserve each file in an `extends` chain
/// as a separate layer, enabling declarative merge strategies (such as append
/// for vectors) to work across the inheritance chain.
///
/// # Parameters
/// - `krate`: Resolved crate path token stream.
/// - `builder_init`: Tokens initializing the `ConfigDiscovery::builder`.
/// - `builder_steps`: Sequence of builder method calls (for example
///   `env_var`, `dotfile_name`).
/// - `cli_chain`: Tokens adding CLI-provided required paths to the builder.
fn build_discovery_loading_block(
    krate: &proc_macro2::TokenStream,
    builder_init: &proc_macro2::TokenStream,
    builder_steps: &[proc_macro2::TokenStream],
    cli_chain: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {{
        let mut builder = #builder_init;
        #(#builder_steps)*
        #cli_chain
        let discovery = builder.build();
        let #krate::discovery::DiscoveryLayersOutcome {
            value: layers,
            mut required_errors,
            mut optional_errors,
        } = discovery.compose_layers();
        errors.append(&mut required_errors);
        if layers.is_empty() {
            errors.append(&mut optional_errors);
        }
        layers
    }}
}

fn build_discovery_based_loading(
    krate: &proc_macro2::TokenStream,
    discovery: &DiscoveryTokens,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    let app_name = syn::LitStr::new(&discovery.app_name, proc_macro2::Span::call_site());
    let env_var = syn::LitStr::new(&discovery.env_var, proc_macro2::Span::call_site());
    let config_file_stmt = build_optional_stmt(
        to_lit_str(discovery.config_file_name.as_ref()),
        "config_file_name",
    );
    let dotfile_stmt =
        build_optional_stmt(to_lit_str(discovery.dotfile_name.as_ref()), "dotfile_name");
    let project_stmt = build_optional_stmt(
        to_lit_str(discovery.project_file_name.as_ref()),
        "project_file_name",
    );
    let cli_chain = build_cli_chain_tokens(has_config_path);
    let builder_init = quote! { #krate::ConfigDiscovery::builder(#app_name) };
    let mut builder_steps = vec![quote! { builder = builder.env_var(#env_var); }];
    if let Some(stmt) = config_file_stmt {
        builder_steps.push(stmt);
    }
    if let Some(stmt) = dotfile_stmt {
        builder_steps.push(stmt);
    }
    if let Some(stmt) = project_stmt {
        builder_steps.push(stmt);
    }
    build_discovery_loading_block(krate, &builder_init, &builder_steps, &cli_chain)
}

pub(crate) fn build_file_discovery(
    tokens: &LoadImplTokens<'_>,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    let krate = tokens.krate;
    tokens.discovery.map_or_else(
        || {
            let app_name =
                syn::LitStr::new(&tokens.legacy_app_name, proc_macro2::Span::call_site());
            let config_env_var = tokens.config_env_var;
            let dotfile_name = tokens.dotfile_name.clone();
            let cli_chain = build_cli_chain_tokens(has_config_path);
            let builder_init = quote! { #krate::ConfigDiscovery::builder(#app_name) };
            let builder_steps = vec![
                quote! { builder = builder.env_var(#config_env_var); },
                quote! { builder = builder.dotfile_name(#dotfile_name); },
            ];
            build_discovery_loading_block(krate, &builder_init, &builder_steps, &cli_chain)
        },
        |discovery| build_discovery_based_loading(krate, discovery, has_config_path),
    )
}

/// Build the environment provider setup.
///
/// Environment variables sit just below CLI arguments in the precedence order,
/// using a prefix if one was supplied in the macro input. See the "Primary data
/// flow" and "The `OrthoConfig` Trait" sections in the design document.
pub(crate) fn build_env_section(tokens: &LoadImplTokens<'_>) -> proc_macro2::TokenStream {
    let env_provider = tokens.env_provider;
    let krate = tokens.krate;
    quote! {
        let env_provider = {
            #env_provider
                // Use runtime re-exports so generated code only requires
                // the crate under its configured name.
                .map(|k| #krate::uncased::Uncased::new(
                    k.as_str().to_ascii_uppercase(),
                ))
                .split("__")
        };
    }
}

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
    let krate = tokens.krate;
    let file_discovery = build_file_discovery(tokens, *has_config_path);
    let env_section = build_env_section(tokens);

    if *profiles {
        load_impl_profiles::build_profile_compose_layers_impl(args, &file_discovery, &env_section)
    } else {
        quote! {
            use clap::Parser as _;
            // Keep this path anchored under the resolved crate so derive users
            // do not need a direct `figment` dependency for macro-generated code.
            use #krate::figment::Figment;
            use #krate::OrthoMergeExt as _;

            let mut errors: Vec<std::sync::Arc<#krate::OrthoError>> = Vec::new();
            let cli = match Self::try_parse_from(iter) {
                Ok(c) => Some(c),
                Err(e) => {
                    errors.push(std::sync::Arc::new(e.into()));
                    None
                }
            };

            let mut composer = #krate::MergeComposer::with_capacity(4);
            let defaults = #defaults_ident { #( #default_struct_init, )* };
            let mut defaults_value = None;
            match #krate::sanitize_value(&defaults) {
                Ok(value) => {
                    defaults_value = Some(value.clone());
                    composer.push_defaults(value);
                }
                Err(err) => errors.push(err),
            }

            let file_layers = #file_discovery;
            for layer in file_layers {
                composer.push_layer(layer);
            }

            #env_section
            match Figment::from(env_provider.clone())
                .extract::<#krate::serde_json::Value>()
                .into_ortho_merge()
            {
                Ok(value) => composer.push_environment(value),
                Err(err) => errors.push(err),
            }

            if let Some(ref cli) = cli {
                match #krate::sanitize_value(cli) {
                    Ok(value) => {
                        let differs_from_defaults = defaults_value
                            .as_ref()
                            .map_or(true, |defaults| defaults != &value);
                        if differs_from_defaults {
                            composer.push_cli(value);
                        }
                    }
                    Err(err) => errors.push(err),
                }
            }

            #krate::declarative::LayerComposition::new(composer.layers(), errors)
        }
    }
}
