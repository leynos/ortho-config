//! Generated CLI parsing and layering fragments for derived configuration loads.

use quote::quote;

/// Generate CLI parsing that retains clap's value-source metadata for layering.
pub(super) fn build_cli_parse_tokens() -> proc_macro2::TokenStream {
    quote! {
        let (cli, matches) = match Self::command().try_get_matches_from_mut(iter) {
            Ok(matches) => match Self::from_arg_matches(&matches) {
                Ok(cli) => (Some(cli), Some(matches)),
                Err(error) => {
                    errors.push(std::sync::Arc::new(error.into()));
                    (None, None)
                }
            },
            Err(error) => {
                errors.push(std::sync::Arc::new(error.into()));
                (None, None)
            }
        };
    }
}

/// Generate CLI layering that omits defaults which clap did not receive explicitly.
pub(super) fn build_cli_layer_tokens(
    krate: &proc_macro2::TokenStream,
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    let sanitized_layer = build_sanitized_cli_layer_tokens(krate, cli_default_as_absent_fields);
    quote! {
        if let (Some(cli), Some(matches)) = (cli.as_ref(), matches.as_ref()) {
            #sanitized_layer
        }
    }
}

/// Generate sanitisation handling for the generated CLI layer.
fn build_sanitized_cli_layer_tokens(
    krate: &proc_macro2::TokenStream,
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    let prune_defaults = build_default_as_absent_pruning_tokens(cli_default_as_absent_fields);
    let push_cli = build_cli_push_tokens();
    quote! {
        match #krate::sanitize_value(cli) {
            Ok(mut value) => {
                #prune_defaults
                #push_cli
            }
            Err(err) => errors.push(err),
        }
    }
}

/// Generate removal of default-derived fields from the generated CLI JSON object.
///
/// The emitted statements expect `matches: &clap::ArgMatches` in scope.
/// Both the declarative and the profile-enabled CLI layers share this pruning
/// so an inferred clap default cannot outrank a profile or file layer.
fn build_default_as_absent_pruning_tokens(
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    quote! {
        let has_explicit_default_as_absent_value = false
            #( || matches.value_source(#cli_default_as_absent_fields)
                == Some(clap::parser::ValueSource::CommandLine) )*;
        if let Some(values) = value.as_object_mut() {
            #(
                if matches.value_source(#cli_default_as_absent_fields)
                    != Some(clap::parser::ValueSource::CommandLine)
                {
                    values.remove(#cli_default_as_absent_fields);
                }
            )*
        }
    }
}

/// Generate the comparison that avoids pushing a CLI layer made only of defaults.
fn build_cli_push_tokens() -> proc_macro2::TokenStream {
    quote! {
        let differs_from_defaults = defaults_value
            .as_ref()
            .map_or(true, |defaults| defaults != &value);
        if differs_from_defaults || has_explicit_default_as_absent_value {
            composer.push_cli(value);
        }
    }
}

/// Generate the CLI layer for structs that opt into profile support.
///
/// The push is gated on clap value sources rather than on the declarative
/// default-as-absent list alone: `--profile` sits below explicit CLI input, so
/// any config field clap reports as command-line *or* environment-filled must
/// still reach the composer. The environment origin matters because the
/// selector may itself be supplied through `<PREFIX>PROFILE`, and in that case
/// the remaining environment-derived fields must keep outranking the profile.
///
/// Fields filled from an inferred clap default are pruned first, so a built-in
/// default cannot mask a profile value.
pub(super) fn build_profile_cli_layer_tokens(
    krate: &proc_macro2::TokenStream,
    cli_arg_ids: &[String],
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    let arg_id_lits: Vec<syn::LitStr> = cli_arg_ids
        .iter()
        .map(|id| syn::LitStr::new(id, proc_macro2::Span::call_site()))
        .collect::<Vec<_>>();
    let prune_defaults = build_default_as_absent_pruning_tokens(cli_default_as_absent_fields);
    // `matches` is already `&ArgMatches` here, courtesy of the `if let` above.
    let has_explicitly_provided = quote! {
        let has_explicitly_provided_value = {
            let provided = [ #( #arg_id_lits ),* ];
            provided.iter().any(|id| {
                matches!(
                    matches.value_source(id),
                    Some(
                        clap::parser::ValueSource::CommandLine
                            | clap::parser::ValueSource::EnvVariable
                    )
                )
            })
        };
    };
    quote! {
        if let (Some(cli), Some(matches)) = (cli.as_ref(), matches.as_ref()) {
            match #krate::sanitize_value(cli) {
                Ok(mut value) => {
                    #prune_defaults
                    #has_explicitly_provided
                    let differs_from_defaults = defaults_value
                        .as_ref()
                        .map_or(true, |defaults| defaults != &value);
                    if differs_from_defaults
                        || has_explicit_default_as_absent_value
                        || has_explicitly_provided_value
                    {
                        composer.push_cli(value);
                    }
                }
                Err(err) => errors.push(err),
            }
        }
    }
}
