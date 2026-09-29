//! Generate configuration file discovery expressions.
//!
//! Explicit and legacy settings share the same builder path. Required CLI
//! paths always contribute errors; optional search errors matter only when no
//! configuration layer is found.

use quote::quote;

use super::{DiscoveryTokens, LoadImplTokens, LoadSourceTokens};

/// Convert an optional configured name into a literal embedded in generated Rust.
fn to_lit_str(value: Option<&String>) -> Option<syn::LitStr> {
    value.map(|contents| syn::LitStr::new(contents, proc_macro2::Span::call_site()))
}

/// Emit a builder assignment for a configured discovery name, omitting absent names.
fn build_optional_stmt(
    lit: Option<syn::LitStr>,
    method_name: &str,
) -> Option<proc_macro2::TokenStream> {
    lit.map(|lit_str| {
        let method_ident = syn::Ident::new(method_name, proc_macro2::Span::call_site());
        quote! { builder = builder.#method_ident(#lit_str); }
    })
}

/// Emit the generated branch that adds a user-selected path as a required source.
///
/// Without the option, no CLI-specific discovery step is generated.
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

/// Generate discovery setup using the explicit application and file names.
///
/// The optional CLI path remains required, while optional search failures are
/// retained only if no configuration layer was found.
fn build_discovery_based_loading(
    krate: &proc_macro2::TokenStream,
    discovery: &DiscoveryTokens,
    has_config_path: bool,
    source_tokens: Option<&LoadSourceTokens<'_>>,
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
    if let Some(injected_sources) = source_tokens {
        let discovery_source = injected_sources.discovery;
        builder_steps.push(quote! { builder = builder.env_source(#discovery_source); });
    }
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

/// Generate file discovery from explicit names or the legacy application defaults.
///
/// Explicit discovery settings select their own environment variable and optional
/// filenames; otherwise the legacy environment variable and dotfile are used.
/// Source-aware entry points pass their lookup-only source into discovery while
/// reserving the separately supplied scan source for environment merging.
pub(super) fn build_file_discovery(
    tokens: &LoadImplTokens<'_>,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    let krate = tokens.krate;
    let source_tokens = tokens.sources.as_ref();
    tokens.discovery.map_or_else(
        || {
            let app_name =
                syn::LitStr::new(&tokens.legacy_app_name, proc_macro2::Span::call_site());
            let config_env_var = tokens.config_env_var;
            let dotfile_name = tokens.dotfile_name.clone();
            let cli_chain = build_cli_chain_tokens(has_config_path);
            let builder_init = quote! { #krate::ConfigDiscovery::builder(#app_name) };
            let mut builder_steps = vec![
                quote! { builder = builder.env_var(#config_env_var); },
                quote! { builder = builder.dotfile_name(#dotfile_name); },
            ];
            if let Some(injected_sources) = source_tokens {
                let discovery_source = injected_sources.discovery;
                builder_steps.push(quote! { builder = builder.env_source(#discovery_source); });
            }
            build_discovery_loading_block(krate, &builder_init, &builder_steps, &cli_chain)
        },
        |discovery| build_discovery_based_loading(krate, discovery, has_config_path, source_tokens),
    )
}
