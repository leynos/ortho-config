//! Helpers for generating the `load_from_iter` implementation.
//!
//! These functions assemble the configuration loading logic by piecing together
//! CLI parsing, file discovery, environment provider setup, and final merging
//! according to the order described in the design document. See
//! [`docs/design.md`](../../docs/design.md) for the high-level architecture.

use quote::quote;
use syn::Ident;

/// Identifiers used when generating the load implementation.
pub(crate) struct LoadImplIdents<'a> {
    pub ident: &'a Ident,
    pub cli_mod: &'a Ident,
    pub cli_ident: &'a Ident,
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
}

/// Generate the `clap` parsing snippet for `load_from_iter`.
///
/// The design document's "Primary data flow" section explains that CLI
/// arguments are parsed first so they can override all other sources.
pub(crate) fn build_cli_parsing(idents: &LoadImplIdents<'_>) -> proc_macro2::TokenStream {
    let LoadImplIdents {
        cli_mod, cli_ident, ..
    } = *idents;
    quote! {
        let cli = #cli_mod::#cli_ident::try_parse_from(
            args.into_iter().map(|a| a.as_ref().to_os_string())
        )
        .map_err(ortho_config::OrthoError::CliParsing)?;
    }
}

/// Generate the file discovery logic section.
///
/// Configuration files are searched in multiple locations as described in the
/// "Configuration File Discovery" section of the design document. This mirrors
/// standard XDG behaviour on Unix-like systems and uses `directories` on Windows.
pub(crate) fn build_file_discovery(tokens: &LoadImplTokens<'_>) -> proc_macro2::TokenStream {
    let LoadImplTokens {
        config_env_var,
        dotfile_name,
        xdg_snippet,
        ..
    } = tokens;
    quote! {
        let mut file_fig = None;
        let candidates = std::iter::empty()
            .chain(cli.config_path.clone())
            .chain(std::env::var_os(#config_env_var).map(std::path::PathBuf::from))
            .chain(Some(std::path::PathBuf::from(#dotfile_name)))
            .chain(std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(#dotfile_name)));
        for path in candidates {
            if let Some(fig) = ortho_config::load_config_file(&path)? {
                file_fig = Some(fig);
                break;
            }
        }
        #xdg_snippet
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

/// Build the merging and final extraction portion of `load_from_iter`.
///
/// Providers are layered as defaults, file, environment, then CLI as described
/// in the design document's "Primary data flow" section.
pub(crate) fn build_merge_section(
    idents: &LoadImplIdents<'_>,
    tokens: &LoadImplTokens<'_>,
) -> proc_macro2::TokenStream {
    let LoadImplIdents { defaults_ident, .. } = *idents;
    let LoadImplTokens {
        default_struct_init,
        override_init_ts,
        append_logic,
        ..
    } = tokens;
    quote! {
        let mut fig = figment::Figment::new();
        let defaults = #defaults_ident { #( #default_struct_init, )* };

        let mut overrides = #override_init_ts;

        fig = fig.merge(figment::providers::Serialized::defaults(&defaults));

        if let Some(ref f) = file_fig {
            fig = fig.merge(f);
        }

        fig = fig.merge(env_provider.clone()).merge(figment::providers::Serialized::from(&cli, figment::Profile::Default));

        #append_logic

        fig = fig.merge(figment::providers::Serialized::defaults(overrides));

        fig.extract().map_err(ortho_config::OrthoError::Gathering)
    }
}

/// Assemble the final `load_from_iter` method using the helper snippets.
pub(crate) fn build_load_impl(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplArgs { idents, tokens } = args;
    let LoadImplIdents { ident, .. } = idents;
    let cli_parsing = build_cli_parsing(idents);
    let file_discovery = build_file_discovery(tokens);
    let env_section = build_env_section(tokens);
    let merge_section = build_merge_section(idents, tokens);

    quote! {
        impl #ident {
            #[allow(dead_code)]
            pub fn load_from_iter<I>(args: I) -> Result<Self, ortho_config::OrthoError>
            where
                I: IntoIterator,
                I::Item: AsRef<std::ffi::OsStr>,
            {
                use clap::Parser as _;
                use figment::{Figment, providers::{Toml, Env, Serialized, Format}, Profile};
                #[cfg(feature = "json5")] use figment_json5::Json5;
                #[cfg(feature = "yaml")] use figment::providers::Yaml;
                use uncased::Uncased;
                #[cfg(feature = "yaml")] use serde_yaml;
                #[cfg(feature = "toml")] use toml;

                #cli_parsing
                #file_discovery
                #env_section
                #merge_section
            }
        }
    }
}
