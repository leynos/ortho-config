//! Generated CLI parsing and layering fragments for derived configuration loads.

use quote::quote;

/// Generate the guard that decides whether the CLI layer may be pushed.
///
/// The CLI layer must be pushed whenever the user supplied *any* argument,
/// regardless of whether the resulting sanitised object happens to equal the
/// defaults object. Comparing whole objects instead silently discards the CLI
/// layer for a single-field configuration whose explicit value matches the
/// struct default, letting a lower-precedence file or environment value win
/// over an explicit `--flag=false`. Ask clap's per-argument `value_source`
/// instead, which reports `CommandLine` independently of the parsed value.
fn build_cli_push_tokens() -> proc_macro2::TokenStream {
    quote! {
        let has_command_line_value = matches.ids().any(|id| {
            matches.value_source(id.as_ref()) == Some(clap::parser::ValueSource::CommandLine)
        });
        if has_command_line_value {
            composer.push_cli(value);
        }
    }
}

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
fn build_default_as_absent_pruning_tokens(
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    quote! {
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

