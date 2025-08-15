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
pub(crate) fn build_file_discovery(
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
        let mut discovery_errors: Vec<ortho_config::OrthoError> = Vec::new();
        #xdg_snippet
        errors.extend(discovery_errors);
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
                .map(|k| uncased::Uncased::new(k.as_str().to_ascii_uppercase()))
                .split("__")
        };
    }
}

/// Build tokens that merge a sanitised CLI provider into a `Figment`.
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
                if errors.is_empty() {
                    Ok(cfg)
                } else {
                    Err(ortho_config::OrthoError::aggregate(errors))
                }
            }
            Err(e) => {
                errors.push(ortho_config::OrthoError::Gathering(e));
                Err(ortho_config::OrthoError::aggregate(errors))
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
            pub fn load_from_iter<I, T>(iter: I) -> Result<#config_ident, ortho_config::OrthoError>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                use clap::Parser as _;
                use figment::{Figment, providers::{Toml, Serialized}, Profile};
                use ortho_config::CsvEnv;
                #[cfg(feature = "json5")] use figment_json5::Json5;
                #[cfg(feature = "yaml")] use figment::providers::Yaml;
                use uncased::Uncased;
                #[cfg(feature = "yaml")] use serde_yaml;
                #[cfg(feature = "toml")] use toml;

                let mut errors: Vec<ortho_config::OrthoError> = Vec::new();
                let cli = match Self::try_parse_from(iter) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        errors.push(ortho_config::OrthoError::CliParsing(e));
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
