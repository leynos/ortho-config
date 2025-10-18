//! Helpers for generating the `load_and_merge` implementation.
//!
//! These functions assemble the configuration loading logic by piecing together
//! CLI parsing, file discovery, environment provider setup, and final merging
//! according to the order described in the design document. See
//! [`docs/design.md`](../../docs/design.md) for the high-level architecture.

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
    pub override_init_ts: &'a proc_macro2::TokenStream,
    pub append_logic: &'a proc_macro2::TokenStream,
    pub config_env_var: &'a proc_macro2::TokenStream,
    pub dotfile_name: &'a syn::LitStr,
    pub legacy_app_name: String,
    pub discovery: Option<&'a DiscoveryTokens>,
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
}

/// CLI parsing is performed outside the generated method.
///
/// Generate the file discovery logic section.
///
/// Configuration files are searched in multiple locations as described in the
/// "Configuration File Discovery" section of the design document. This mirrors
/// standard XDG behaviour on Unix-like systems and uses `directories` on Windows.
fn to_lit_str(value: Option<&String>) -> Option<syn::LitStr> {
    value.map(|value| syn::LitStr::new(value, proc_macro2::Span::call_site()))
}

fn build_optional_stmt(
    lit: Option<syn::LitStr>,
    method_name: &str,
) -> Option<proc_macro2::TokenStream> {
    lit.map(|lit| {
        let method_ident = syn::Ident::new(method_name, proc_macro2::Span::call_site());
        quote! { builder = builder.#method_ident(#lit); }
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
/// # Parameters
/// - `builder_init`: Tokens initialising the `ConfigDiscovery::builder`.
/// - `builder_steps`: Sequence of builder method calls (for example
///   `env_var`, `dotfile_name`).
/// - `cli_chain`: Tokens adding CLI-provided required paths to the builder.
///
/// # Examples
/// ```ignore
/// use proc_macro2::TokenStream;
/// use quote::quote;
///
/// let block = build_discovery_loading_block(
///     &quote! { ortho_config::ConfigDiscovery::builder("app") },
///     &[quote! { builder = builder.env_var("APP_CONFIG"); }],
///     &TokenStream::new(),
/// );
/// assert!(block.to_string().contains("required_errors"));
/// ```
fn build_discovery_loading_block(
    builder_init: &proc_macro2::TokenStream,
    builder_steps: &[proc_macro2::TokenStream],
    cli_chain: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        let mut file_fig = None;
        let mut builder = #builder_init;
        #(#builder_steps)*
        #cli_chain
        let discovery = builder.build();
        let ortho_config::DiscoveryLoadOutcome {
            figment: loaded_fig,
            mut required_errors,
            mut optional_errors,
        } = discovery.load_first_partitioned();
        if let Some(fig) = loaded_fig {
            file_fig = Some(fig);
        }
        errors.append(&mut required_errors);
        if file_fig.is_none() {
            errors.append(&mut optional_errors);
        }
    }
}

fn build_discovery_based_loading(
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
    let builder_init = quote! { ortho_config::ConfigDiscovery::builder(#app_name) };
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
    build_discovery_loading_block(&builder_init, &builder_steps, &cli_chain)
}

pub(crate) fn build_file_discovery(
    tokens: &LoadImplTokens<'_>,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    if let Some(discovery) = tokens.discovery {
        build_discovery_based_loading(discovery, has_config_path)
    } else {
        let app_name = syn::LitStr::new(&tokens.legacy_app_name, proc_macro2::Span::call_site());
        let config_env_var = tokens.config_env_var;
        let dotfile_name = tokens.dotfile_name.clone();
        let cli_chain = build_cli_chain_tokens(has_config_path);
        let builder_init = quote! { ortho_config::ConfigDiscovery::builder(#app_name) };
        let builder_steps = vec![
            quote! { builder = builder.env_var(#config_env_var); },
            quote! { builder = builder.dotfile_name(#dotfile_name); },
        ];
        build_discovery_loading_block(&builder_init, &builder_steps, &cli_chain)
    }
}

/// Build the environment provider setup.
///
/// Environment variables sit just below CLI arguments in the precedence order,
/// using a prefix if one was supplied in the macro input. See the "Primary data
/// flow" and "The `OrthoConfig` Trait" sections in the design document.
pub(crate) fn build_env_section(tokens: &LoadImplTokens<'_>) -> proc_macro2::TokenStream {
    let env_provider = tokens.env_provider;
    quote! {
        let env_provider = {
            #env_provider
                .map(|k| ortho_config::uncased::Uncased::new(
                    k.as_str().to_ascii_uppercase(),
                ))
                .split("__")
        };
    }
}

/// Build tokens that merge a sanitized CLI provider into a `Figment`.
///
/// The generated code merges the provider when present and pushes any
/// resulting errors onto the supplied collection.
///
/// # Examples
/// ```ignore
/// use quote::quote;
/// let fig = quote!(fig);
/// let errors = quote!(errors);
/// let tokens = merge_cli_provider_tokens(&fig, &errors);
/// # let _ = tokens;
/// ```
fn merge_cli_provider_tokens(
    fig_var: &proc_macro2::TokenStream,
    errors_var: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        if let Some(ref cli) = cli {
            match ortho_config::sanitized_provider(cli) {
                Ok(p) => #fig_var = #fig_var.merge(p),
                Err(e) => #errors_var.push(e),
            }
        }
    }
}

/// Build the merging and final extraction portion of `load_and_merge`.
///
/// Providers are layered as defaults, file, environment, then CLI as described
/// in the design document's "Primary data flow" section.
pub(crate) fn build_merge_section(
    idents: &LoadImplIdents<'_>,
    tokens: &LoadImplTokens<'_>,
) -> proc_macro2::TokenStream {
    let LoadImplIdents {
        defaults_ident,
        config_ident,
        ..
    } = *idents;
    let LoadImplTokens {
        default_struct_init,
        override_init_ts,
        append_logic,
        ..
    } = tokens;
    let fig_ts = quote!(fig);
    let errors_ts = quote!(errors);
    let cli_merge = merge_cli_provider_tokens(&fig_ts, &errors_ts);
    quote! {
        let mut fig = Figment::new();
        let defaults = #defaults_ident { #( #default_struct_init, )* };

        let mut overrides = #override_init_ts;

        fig = fig.merge(Serialized::defaults(&defaults));

        if let Some(ref f) = file_fig {
            fig = fig.merge(f);
        }
        let env_figment = Figment::from(env_provider);
        fig = fig.merge(env_figment.clone());
        #cli_merge

        #append_logic

        match ortho_config::sanitized_provider(&overrides) {
            Ok(p) => fig = fig.merge(p),
            Err(e) => errors.push(e),
        }

        match fig.extract::<#config_ident>() {
            Ok(cfg) => {
                if errors.is_empty() { Ok(cfg) }
                else if errors.len() == 1 { Err(errors.pop().expect("one error")) }
                else { Err(ortho_config::OrthoError::aggregate(errors).into()) }
            }
            Err(e) => {
                errors.push(std::sync::Arc::new(ortho_config::OrthoError::merge(e)));
                Err(ortho_config::OrthoError::aggregate(errors).into())
            }
        }
    }
}

/// Assemble the final `load_from_iter` method using the helper snippets.
pub(crate) fn build_load_impl(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplArgs {
        idents,
        tokens,
        has_config_path,
    } = args;
    let LoadImplIdents {
        cli_ident,
        config_ident,
        ..
    } = idents;
    let file_discovery = build_file_discovery(tokens, *has_config_path);
    let env_section = build_env_section(tokens);
    let merge_section = build_merge_section(idents, tokens);

    quote! {
        impl #cli_ident {
            #[expect(dead_code, reason = "Generated method may not be used in all builds")]
            pub fn load_from_iter<I, T>(iter: I) -> ortho_config::OrthoResult<#config_ident>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                use clap::Parser as _;
                use ortho_config::figment::{providers::Serialized, Figment};

                let mut errors: Vec<std::sync::Arc<ortho_config::OrthoError>> = Vec::new();
                let cli = match Self::try_parse_from(iter) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        errors.push(std::sync::Arc::new(e.into()));
                        None
                    }
                };

                #file_discovery
                #env_section
                #merge_section
            }
        }
    }
}
