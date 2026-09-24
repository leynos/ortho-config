//! Profile-enabled compose body and entry-point helpers (roadmap 9.1.1).
//!
//! The compose body for opted-in structs resolves the selection from the
//! parsed CLI (or the environment when clap parsing failed), extracts profile
//! tables from the same discovered layers, strips the selector from the
//! environment and CLI layers, and gates the CLI push on clap value-source
//! information so an explicit flag equal to the default still beats the
//! profile (risk 3).

use quote::quote;
use syn::Ident;

use crate::derive::load_impl::LoadImplArgs;
use crate::derive::load_impl::LoadSourceTokens;
use crate::derive::load_impl::cli::build_profile_cli_layer_tokens;

pub(crate) fn build_profile_compose_layers_impl(
    args: &LoadImplArgs<'_>,
    file_discovery: &proc_macro2::TokenStream,
    env_section: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let LoadImplArgs {
        tokens,
        profile_env_var,
        cli_arg_ids,
        ..
    } = args;
    let krate = tokens.krate;
    let cli_default_as_absent_fields = &tokens.default_struct_init.cli_default_as_absent_fields;
    let parse_setup = build_profile_parse_setup(krate);
    let selection = build_profile_selection(krate, profile_env_var, tokens.sources.as_ref());
    let defaults = build_profile_defaults(args);
    let file_layers = build_profile_file_layers(krate, file_discovery);
    let environment_layer = build_profile_environment_layer(krate, env_section);
    let cli_push = build_profile_cli_layer_tokens(krate, cli_arg_ids, cli_default_as_absent_fields);

    quote! {
        // Keep this path anchored under the resolved crate so derive users
        // do not need a direct `figment` dependency for macro-generated code.
        use #krate::figment::Figment;
        use #krate::OrthoMergeExt as _;

        #parse_setup
        #selection
        #defaults
        #file_layers
        #environment_layer

        #cli_push

        let selection_vec: Vec<#krate::SelectedProfile> = selected.into_iter().collect();
        (
            #krate::declarative::LayerComposition::new(composer.layers(), errors),
            selection_vec,
        )
    }
}

/// Build the clap parse setup used by the profile-enabled compose body.
///
/// The `matches` value is retained so the selection can attribute the
/// `--profile` flag to the command line only, and so the CLI layer can consult
/// per-field value sources.
fn build_profile_parse_setup(krate: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! {
        use clap::{CommandFactory as _, FromArgMatches as _, Parser as _};

        let mut errors: Vec<std::sync::Arc<#krate::OrthoError>> = Vec::new();
        let args: Vec<std::ffi::OsString> = iter.into_iter().map(Into::into).collect();
        let matches = match Self::command().try_get_matches_from(args) {
            Ok(matches) => Some(matches),
            Err(err) => {
                errors.push(std::sync::Arc::new(err.into()));
                None
            }
        };
        let cli = match &matches {
            Some(matches) => match Self::from_arg_matches(matches) {
                Ok(cli) => Some(cli),
                Err(err) => {
                    errors.push(std::sync::Arc::new(err.into()));
                    None
                }
            },
            None => None,
        };
    }
}

/// Build the selection resolution for the profile-enabled compose body.
///
/// The flag counts only when clap reports a command-line origin, so an
/// env-filled value stays attributed to the environment variable; when clap
/// parsing failed the environment is read directly so selection errors never
/// mask parse errors.
///
/// When the caller injected a discovery source, the selector is read through
/// that source so selection honours the same environment the file discovery
/// saw. Only the process-backed entry points fall back to `std::env::var`.
fn build_profile_selection(
    krate: &proc_macro2::TokenStream,
    profile_env_var: &str,
    sources: Option<&LoadSourceTokens<'_>>,
) -> proc_macro2::TokenStream {
    let selector_env = syn::LitStr::new(profile_env_var, proc_macro2::Span::call_site());
    // `into_string` mirrors `std::env::var` by treating a non-Unicode value as
    // absent rather than as a selection error.
    let read_env = sources.map_or_else(
        || quote! { std::env::var(#selector_env).ok() },
        |injected| {
            let discovery_source = injected.discovery;
            quote! {
                #discovery_source
                    .get(#selector_env)
                    .and_then(|value| value.into_string().ok())
            }
        },
    );
    quote! {
        // Resolve the selection. The flag counts only when clap reports a
        // command-line origin, so an env-filled value stays attributed to the
        // environment variable; when clap parsing failed the environment is
        // read directly so selection errors never mask parse errors.
        let selected = {
            let flag_value = matches.as_ref().and_then(|m| {
                if m.value_source("profile")
                    == Some(clap::parser::ValueSource::CommandLine)
                {
                    cli.as_ref().and_then(|c| c.profile.as_deref())
                } else {
                    None
                }
            });
            let env_value = #read_env;
            match #krate::SelectedProfile::resolve(flag_value, env_value.as_deref()) {
                Ok(selection) => selection,
                Err(err) => {
                    errors.push(err);
                    None
                }
            }
        };
    }
}

/// Build the defaults layer for the profile-enabled compose body.
///
/// Fallible `cli_default_as_absent` resolutions run before the defaults struct
/// is built, so an inferred clap default is replayed through the parser rather
/// than substituted directly for the field value.
fn build_profile_defaults(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplArgs { idents, tokens, .. } = args;
    let defaults_ident = idents.defaults_ident;
    let default_struct_init = tokens.default_struct_init;
    let default_resolutions = &default_struct_init.resolutions;
    let default_fields = &default_struct_init.fields;
    let krate = tokens.krate;
    quote! {
        let mut composer = #krate::MergeComposer::with_capacity(5);
        #(#default_resolutions)*
        let defaults = #defaults_ident { #( #default_fields, )* };
        let mut defaults_value = None;
        match #krate::sanitize_value(&defaults) {
            Ok(value) => {
                defaults_value = Some(value.clone());
                composer.push_defaults(value);
            }
            Err(err) => errors.push(err),
        }
    }
}

fn build_profile_file_layers(
    krate: &proc_macro2::TokenStream,
    file_discovery: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        let file_layers = #file_discovery;
        match #krate::profile::extract_profile_layers(file_layers, selected.as_ref()) {
            Ok(outcome) => {
                for layer in outcome.file_layers {
                    composer.push_layer(layer);
                }
                for layer in outcome.profile_layers {
                    composer.push_layer(layer);
                }
            }
            Err(err) => errors.push(err),
        }
    }
}

fn build_profile_environment_layer(
    krate: &proc_macro2::TokenStream,
    env_section: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #env_section
        match Figment::from(env_provider.clone())
            .extract::<#krate::serde_json::Value>()
            .into_ortho_merge()
        {
            Ok(mut value) => {
                // The selector must never leak into the merged value.
                if let Some(object) = value.as_object_mut() {
                    object.remove("profile");
                }
                composer.push_environment(value);
            }
            Err(err) => errors.push(err),
        }
    }
}

pub(crate) fn build_config_profile_delegates(
    krate: &proc_macro2::TokenStream,
    cli_ident: &Ident,
    config_ident: &Ident,
) -> proc_macro2::TokenStream {
    quote! {
        impl #config_ident {
            /// Load configuration and report the selected profile.
            pub fn load_with_profile_from_iter<I, T>(iter: I) -> #krate::OrthoResult<#krate::profile::ProfileLoadOutcome<Self>>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #cli_ident::load_with_profile_from_iter(iter)
            }

            /// Load configuration using the current process arguments and
            /// report the selected profile.
            pub fn load_with_profile() -> #krate::OrthoResult<#krate::profile::ProfileLoadOutcome<Self>> {
                #cli_ident::load_with_profile()
            }
        }
    }
}
