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

#[expect(
    clippy::too_many_lines,
    reason = "The generated compose body is a single flat sequence; splitting it would obscure the precedence order"
)]
pub(crate) fn build_profile_compose_layers_impl(
    args: &LoadImplArgs<'_>,
    file_discovery: &proc_macro2::TokenStream,
    env_section: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let LoadImplArgs {
        idents,
        tokens,
        profile_env_var,
        cli_arg_ids,
        ..
    } = args;
    let defaults_ident = idents.defaults_ident;
    let default_struct_init = tokens.default_struct_init;
    let krate = tokens.krate;
    let selector_env = syn::LitStr::new(profile_env_var, proc_macro2::Span::call_site());
    let cli_push = build_profile_cli_push(krate, cli_arg_ids);

    quote! {
        use clap::Parser as _;
        use clap::CommandFactory as _;
        use clap::FromArgMatches as _;
        // Keep this path anchored under the resolved crate so derive users
        // do not need a direct `figment` dependency for macro-generated code.
        use #krate::figment::Figment;
        use #krate::OrthoMergeExt as _;

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
            let env_value = std::env::var(#selector_env).ok();
            match #krate::SelectedProfile::resolve(flag_value, env_value.as_deref()) {
                Ok(selection) => selection,
                Err(err) => {
                    errors.push(err);
                    None
                }
            }
        };

        let mut composer = #krate::MergeComposer::with_capacity(5);
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

        #cli_push

        let selection_vec: Vec<#krate::SelectedProfile> = selected.into_iter().collect();
        (
            #krate::declarative::LayerComposition::new(composer.layers(), errors),
            selection_vec,
        )
    }
}

/// Build the value-source-gated CLI push for opted-in structs.
///
/// The CLI layer is pushed when the sanitized value differs from defaults or
/// when any config-field argument was explicitly provided (command line or
/// environment), so an explicit flag equal to the default still beats the
/// profile (risk 3).
fn build_profile_cli_push(
    krate: &proc_macro2::TokenStream,
    cli_arg_ids: &[String],
) -> proc_macro2::TokenStream {
    let arg_id_lits: Vec<syn::LitStr> = cli_arg_ids
        .iter()
        .map(|id| syn::LitStr::new(id, proc_macro2::Span::call_site()))
        .collect();
    quote! {
        if let Some(ref cli) = cli {
            match #krate::sanitize_value(cli) {
                Ok(value) => {
                    let differs_from_defaults = defaults_value
                        .as_ref()
                        .map_or(true, |defaults| defaults != &value);
                    let explicitly_provided = matches.as_ref().is_some_and(|m| {
                        let provided = [ #( #arg_id_lits ),* ];
                        provided.iter().any(|id| {
                            matches!(
                                m.value_source(id),
                                Some(
                                    clap::parser::ValueSource::CommandLine
                                        | clap::parser::ValueSource::EnvVariable
                                )
                            )
                        })
                    });
                    if differs_from_defaults || explicitly_provided {
                        composer.push_cli(value);
                    }
                }
                Err(err) => errors.push(err),
            }
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
