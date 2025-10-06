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
    pub dotfile_name: &'a proc_macro2::TokenStream,
    pub xdg_snippet: &'a proc_macro2::TokenStream,
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
fn opt_to_litstr(value: Option<&String>) -> Option<syn::LitStr> {
    value.map(|value| syn::LitStr::new(value, proc_macro2::Span::call_site()))
}

fn builder_stmt(lit: Option<syn::LitStr>, method: &str) -> Option<proc_macro2::TokenStream> {
    lit.map(|lit| {
        let method_ident = syn::Ident::new(method, proc_macro2::Span::call_site());
        quote! { builder = builder.#method_ident(#lit); }
    })
}

fn build_legacy_file_discovery(
    tokens: &LoadImplTokens<'_>,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    let LoadImplTokens {
        config_env_var,
        dotfile_name,
        xdg_snippet,
        ..
    } = tokens;
    let config_path_chain = if has_config_path {
        quote! { .chain(cli.as_ref().and_then(|c| c.config_path.clone())) }
    } else {
        quote! {}
    };
    quote! {
        let mut file_fig = None;
        let candidates = std::iter::empty()
            #config_path_chain
            .chain(
                std::env::var_os(#config_env_var)
                    .map(std::path::PathBuf::from),
            )
            .chain(Some(std::path::PathBuf::from(#dotfile_name)))
            .chain(
                std::env::var_os("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(#dotfile_name)),
            );
        for path in candidates {
            match ortho_config::load_config_file(&path) {
                Ok(Some(fig)) => {
                    file_fig = Some(fig);
                    break;
                }
                Ok(None) => {}
                Err(e) => errors.push(e),
            }
        }
        let mut discovery_errors: Vec<std::sync::Arc<ortho_config::OrthoError>> = Vec::new();
        #xdg_snippet
        errors.extend(discovery_errors);
    }
}

pub(crate) fn build_file_discovery(
    tokens: &LoadImplTokens<'_>,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    if let Some(discovery) = tokens.discovery {
        let app_name = syn::LitStr::new(&discovery.app_name, proc_macro2::Span::call_site());
        let env_var = syn::LitStr::new(&discovery.env_var, proc_macro2::Span::call_site());
        let config_file = opt_to_litstr(discovery.config_file_name.as_ref());
        let dotfile = opt_to_litstr(discovery.dotfile_name.as_ref());
        let project_file = opt_to_litstr(discovery.project_file_name.as_ref());
        let config_file_stmt = builder_stmt(config_file, "config_file_name");
        let dotfile_stmt = builder_stmt(dotfile, "dotfile_name");
        let project_stmt = builder_stmt(project_file, "project_file_name");
        let cli_chain = if has_config_path {
            quote! {
                if let Some(ref cli) = cli {
                    if let Some(ref path) = cli.config_path {
                        builder = builder.add_explicit_path(path.clone());
                    }
                }
            }
        } else {
            quote! {}
        };
        quote! {
            let mut file_fig = None;
            let mut builder = ortho_config::ConfigDiscovery::builder(#app_name);
            builder = builder.env_var(#env_var);
            #config_file_stmt
            #dotfile_stmt
            #project_stmt
            #cli_chain
            let discovery = builder.build();
            match discovery.load_first() {
                Ok(Some(fig)) => file_fig = Some(fig),
                Ok(None) => {},
                Err(e) => errors.push(e),
            }
        }
    } else {
        build_legacy_file_discovery(tokens, has_config_path)
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
